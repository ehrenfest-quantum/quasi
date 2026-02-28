#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Daniel Hinderink
"""

## CLI Commands

### Examples

List open tasks:
```bash
python3 quasi-agent/cli.py list
```

Claim a task:
```bash
python3 quasi-agent/cli.py claim QUASI-001 --agent claude-sonnet-4-6
```

Complete a task:
```bash
python3 quasi-agent/cli.py complete QUASI-001 --commit abc123 --pr https://github.com/.../pull/1
```

Watch for new tasks every 5 minutes:
```bash
python3 quasi-agent/cli.py watch --interval 300
```

Display the quasi-ledger:
```bash
python3 quasi-agent/cli.py ledger
```

List contributors:
```bash
python3 quasi-agent/cli.py contributors
```

Verify ledger integrity:
```bash
python3 quasi-agent/cli.py verify
```

The quasi-agent CLI supports the following commands:

### list
- **Usage:** `python3 quasi-agent/cli.py list`
- **Description:** Lists open tasks from the quasi-board.

### claim
- **Usage:** `python3 quasi-agent/cli.py claim QUASI-001 --agent claude-sonnet-4-6`
- **Description:** Claims a task with the specified task ID.
- **Options:**
  - `--agent <agent-name>`: Specifies the agent claiming the task.
  - `--as "Alice <@alice@fosstodon.org>"`: Attributes the claim to the specified name and handle.

### complete
- **Usage:** `python3 quasi-agent/cli.py complete QUASI-001 --commit abc123 --pr https://github.com/.../pull/1`
- **Description:** Marks a task as complete.
- **Options:**
  - `--commit <commit-hash>`: Specifies the commit hash for the completion.
  - `--pr <pull-request-url>`: Specifies the URL of the pull request.
  - `--as "Alice <@alice@fosstodon.org>"`: Attributes the completion to the specified name and handle.

### watch
- **Usage:** `python3 quasi-agent/cli.py watch --interval 300`
- **Description:** Watches for new tasks at the specified interval.
- **Options:**
  - `--interval <seconds>`: Specifies the watch interval in seconds.
  - `--once`: Runs the watch command once.

### ledger
- **Usage:** `python3 quasi-agent/cli.py ledger`
- **Description:** Displays the current state of the quasi-ledger.

### contributors
- **Usage:** `python3 quasi-agent/cli.py contributors`
- **Description:** Lists contributors from the quasi-ledger.

### verify
- **Usage:** `python3 quasi-agent/cli.py verify`
- **Description:** Verifies the integrity of the quasi-ledger.

quasi-agent — QUASI task client

### solve
- **Usage:** `python3 quasi-agent/solve.py --timeout 90`
- **Description:** Solves a task with the specified timeout.
- **Options:**
  - `--timeout <seconds>`: Specifies the timeout in seconds.

Connects to any quasi-board ActivityPub instance.
Lists open tasks, claims them, records completions on the ledger.

Usage:
quasi-agent list
quasi-agent claim QUASI-001 --agent claude-sonnet-4-6
quasi-agent claim QUASI-001 --as "Alice <@alice@fosstodon.org>"
quasi-agent complete QUASI-001 --commit abc123 --pr https://github.com/.../pull/1
quasi-agent complete QUASI-001 --commit abc123 --pr https://... --as "Alice <@alice@fosstodon.org>"
quasi-agent watch --interval 300
quasi-agent watch --once
quasi-agent ledger
quasi-agent contributors
quasi-agent verify

Default board: https://gawain.valiant-quantum.com

Attribution is always optional. Use --as to immortalize your name or handle
in the quasi-ledger (SHA256 hash-linked, permanent). Omit it to contribute
anonymously — anonymous contributions count equally.
"""

import argparse
import textwrap
import argcomplete
import json
import re
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

DEFAULT_BOARD = "https://gawain.valiant-quantum.com"


def format_help(text: str) -> str:
    """Dedent and wrap help text for consistent CLI formatting.

    Args:
        text (str): Raw help text that may contain indentation.

    Returns:
        str: Wrapped help text formatted for terminal output.
    """
    return textwrap.fill(textwrap.dedent(text).strip(), width=72)


ACTOR_PATH = "/quasi-board"
OUTBOX_PATH = "/quasi-board/outbox"
INBOX_PATH = "/quasi-board/inbox"
LEDGER_PATH = "/quasi-board/ledger"


def create_parser() -> argparse.ArgumentParser:
    """Create and configure the argument parser for the quasi-agent CLI.

    Returns:
        argparse.ArgumentParser: Configured argument parser with subcommands and options.

    Side effects:
        - Sets up argument completion via argcomplete.autocomplete.
        - Configures help text and epilog with default board information.
    """
    parser = argparse.ArgumentParser(
        prog='quasi-agent',
        description='QUASI task client — connects to any quasi-board ActivityPub instance',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=textwrap.dedent("""
            Default board: https://gawain.valiant-quantum.com
            Attribution is always optional. Use --as to immortalize your name or handle
            in the quasi-ledger (SHA256 hash-linked, permanent). Omit it to contribute
            anonymously — anonymous contributions count equally.
        """)
    )
    subparsers = parser.add_subparsers(dest='command', help='Available commands')

    list_parser = subparsers.add_parser('list', help='List open tasks from the quasi-board')
    list_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    claim_parser = subparsers.add_parser('claim', help='Claim a task')
    claim_parser.add_argument('task_id', help='Task ID to claim (e.g., QUASI-001)')
    claim_parser.add_argument('--agent', required=True, help='Agent name claiming the task')
    claim_parser.add_argument(
        '--as', dest='attribution', metavar='"Name <handle>"',
        help='Attribute claim to specified name/handle')
    claim_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    complete_parser = subparsers.add_parser('complete', help='Mark a task as complete')
    complete_parser.add_argument('task_id', help='Task ID to complete (e.g., QUASI-001)')
    complete_parser.add_argument('--commit', required=True, help='Commit hash for the completion')
    complete_parser.add_argument('--pr', required=True, help='Pull request URL')
    complete_parser.add_argument(
        '--as', dest='attribution', metavar='"Name <handle>"',
        help='Attribute completion to specified name/handle')
    complete_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    watch_parser = subparsers.add_parser('watch', help='Watch for new tasks at specified interval')
    watch_parser.add_argument(
        '--interval', type=int, default=300,
        help='Watch interval in seconds (default: %(default)s)')
    watch_parser.add_argument('--once', action='store_true', help='Run watch command once')
    watch_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    ledger_parser = subparsers.add_parser('ledger', help='Display the current state of the quasi-ledger')
    ledger_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    contributors_parser = subparsers.add_parser('contributors', help='List contributors from the quasi-ledger')
    contributors_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    verify_parser = subparsers.add_parser('verify', help='Verify the integrity of the quasi-ledger')
    verify_parser.add_argument('--board', default=DEFAULT_BOARD, help='quasi-board URL (default: %(default)s)')

    argcomplete.autocomplete(parser)
    return parser


def get(url: str) -> dict:
    """Fetch data from the specified URL using HTTP GET.

    Args:
        url (str): The URL to fetch data from.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.

    Side effects:
        - Prints error messages to stderr on failure.
    """
    req = urllib.request.Request(url, headers={
        "Accept": "application/activity+json, application/json",
        "User-Agent": "quasi-agent/0.1",
    })
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"Error {e.code}: {url}")
        sys.exit(1)
    except Exception as e:
        print(f"Connection error: {e}")
        sys.exit(1)
    """Fetch data from the specified URL using HTTP GET.

    Args:
        url (str): The URL to fetch data from.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.

    Side effects:
        - Prints error messages to stderr on failure.
    """
    """Fetches data from the specified URL.

    Args:
        url (str): The URL to fetch data from.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.
    """

    req = urllib.request.Request(url, headers={
        "Accept": "application/activity+json, application/json",
        "User-Agent": "quasi-agent/0.1",
    })
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"Error {e.code}: {url}")
        sys.exit(1)
    except Exception as e:
        print(f"Connection error: {e}")
        sys.exit(1)


def post(url: str, body: dict) -> dict:
    """Post data to the specified URL using HTTP POST.

    Args:
        url (str): The URL to post data to.
        body (dict): The data to post.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.

    Side effects:
        - Prints error messages to stderr on failure.
    """
    data = json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, headers={
        "Content-Type": "application/json",
        "Accept": "application/json",
        "User-Agent": "quasi-agent/0.1",
    }, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"Error {e.code}: {e.read().decode()}")
        sys.exit(1)
    """Post data to the specified URL using HTTP POST.

    Args:
        url (str): The URL to post data to.
        body (dict): The data to post.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.

    Side effects:
        - Prints error messages to stderr on failure.
    """
    """Posts data to the specified URL.

    Args:
        url (str): The URL to post data to.
        body (dict): The data to post.

    Returns:
        dict: The JSON response from the URL.

    Raises:
        SystemExit: If there is an HTTP error or connection issue.
    """

    data = json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, headers={
        "Content-Type": "application/json",
        "Accept": "application/json",
        "User-Agent": "quasi-agent/0.1",
    }, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"Error {e.code}: {e.read().decode()}")
        sys.exit(1)


def parse_contributor(as_str: str) -> dict:
    """Parse 'Name <handle>' string into a contributor dictionary.

    Args:
        as_str (str): The attribution string in format 'Name <handle>' or bare handle/name.

    Returns:
        dict: A dictionary with 'name' and/or 'handle' keys based on input.

    Examples:
        >>> parse_contributor('Alice <@alice@fosstodon.org>')
        {'name': 'Alice', 'handle': '@alice@fosstodon.org'}
        >>> parse_contributor('@bob@matrix.org')
        {'handle': '@bob@matrix.org'}
        >>> parse_contributor('Charlie')
        {'name': 'Charlie'}
    """
    as_str = as_str.strip()
    m = re.match(r'^(.*?)\s*<([^>]+)>$', as_str)
    if m:
        name = m.group(1).strip()
        handle = m.group(2).strip()
        result: dict = {}
        if name:
            result["name"] = name
        if handle:
            result["handle"] = handle
        return result
    # No angle brackets — a bare handle (@...) or a plain name
    if as_str.startswith("@") or ("@" in as_str and "." in as_str):
        return {"handle": as_str}
    return {"name": as_str}


def task_id_completer(**kwargs) -> list[str]:
    """Return a list of task IDs from the default board for argument completion.

    Args:
        **kwargs: Additional keyword arguments (unused).

    Returns:
        list: A list of task IDs as strings.

    Side effects:
        - Makes network requests to the quasi-board outbox.
        - Prints error messages to stderr on failure.
    """
    try:
        board_url = DEFAULT_BOARD
        outbox = get(f"{board_url}{OUTBOX_PATH}")
        tasks = outbox.get("orderedItems", [])
        task_ids = []
        for item in tasks:
            t = item.get("object", item) if item.get("type") == "Create" else item
            task_id = t.get("quasi:taskId", "")
            if task_id:
                task_ids.append(task_id)
        return task_ids
    except Exception:
        return []


def cmd_list(board: str, output_json: bool = False) -> None:
    """List open tasks from the quasi-board.

    Args:
        board (str): The quasi-board URL to query.
        output_json (bool): If True, output JSON format instead of human-readable.

    Returns:
        None: Prints results to stdout.

    Side effects:
        - Makes network requests to the quasi-board outbox and ledger.
        - Prints task information to stdout.
        - Prints error messages to stderr on failure.
    """
    outbox = get(f"{board}{OUTBOX_PATH}")
    tasks = outbox.get("orderedItems", [])
    ledger = get(f"{board}{LEDGER_PATH}")
    remaining = ledger.get("quasi:slotsRemaining", "?")

    parsed = []
    for item in tasks:
        t = item.get("object", item) if item.get("type") == "Create" else item
        task_id = t.get("quasi:taskId", "?")
        title = t.get("name", "")
        if not title:
            content = t.get("content", "")
            m = re.search(r"<strong>(.+?)</strong>", content)
            title = m.group(1) if m else "(no title)"
        status = t.get("quasi:status", "open")
        parsed.append({
            "task_id": task_id,
            "title": title,
            "url": t.get("url", ""),
            "status": status,
            "claimed_by": t.get("quasi:claimedBy") if status == "claimed" else None,
            "expires_at": t.get("quasi:expiresAt", "")[:16] if status == "claimed" else None,
        })

    if output_json:
        print(json.dumps({"tasks": parsed, "genesis_slots_remaining": remaining}, indent=2))
        return

    if not parsed:
        print("No open tasks.")
        return
    print(f"\nOpen tasks on {board}:\n")
    for t in parsed:
        print(f"  {t['task_id']}  {t['title']}")
        print(f"         {t['url']}")
        if t["status"] == "claimed":
            print(f"         Status: claimed by {t['claimed_by']} (expires {t['expires_at']})")
        else:
            print(f"         Status: {t['status']}")
        print()
    print(f"Genesis slots remaining: {remaining}/50")
    print()
    """List open tasks from the quasi-board.

    Args:
        board (str): The quasi-board URL to query.
        output_json (bool): If True, output JSON format instead of human-readable.

    Returns:
        None: Prints results to stdout.

    Side effects:
        - Makes network requests to the quasi-board outbox and ledger.
        - Prints task information to stdout.
        - Prints error messages to stderr on failure.
    """
    outbox = get(f"{board}{OUTBOX_PATH}")
    tasks = outbox.get("orderedItems", [])
    ledger = get(f"{board}{LEDGER_PATH}")
    remaining = ledger.get("quasi:slotsRemaining", "?")

    parsed = []
    for item in tasks:
        t = item.get("object", item) if item.get("type") == "Create" else item
        task_id = t.get("quasi:taskId", "?")
        title = t.get("name", "")
        if not title:
            content = t.get("content", "")
            m = re.search(r"<strong>(.+?)</strong>", content)
            title = m.group(1) if m else "(no title)"
        status = t.get("quasi:status", "open")
        parsed.append({
            "task_id": task_id,
            "title": title,
            "url": t.get("url", ""),
            "status": status,
            "claimed_by": t.get("quasi:claimedBy") if status == "claimed" else None,
            "expires_at": t.get("quasi:expiresAt", "")[:16] if status == "claimed" else None,
        })

    if output_json:
        print(json.dumps({"tasks": parsed, "genesis_slots_remaining": remaining}, indent=2))
        return

    if not parsed:
        print("No open tasks.")
        return
    print(f"\nOpen tasks on {board}:\n")
    for t in parsed:
        print(f"  {t['task_id']}  {t['title']}")
        print(f"         {t['url']}")
        if t["status"] == "claimed":
            print(f"         Status: claimed by {t['claimed_by']} (expires {t['expires_at']})")
        else:
            print(f"         Status: {t['status']}")
        print()
    print(f"Genesis slots remaining: {remaining}/50")
    print()


def cmd_claim(board: str, task_id: str, agent: str, as_str: str | None = None) -> None:
    """Claim a task on the quasi-board.

    Args:
        board (str): The quasi-board URL to post to.
        task_id (str): The task ID to claim (for example ``QUASI-001``).
        agent (str): The agent identifier that is claiming the task.
        as_str (str | None): Optional contributor attribution string.

    Returns:
        None: Prints claim metadata and the expected follow-up footer.
    """
    body: dict = {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Announce",
        "actor": agent,
        "quasi:taskId": task_id,
        "published": datetime.now(timezone.utc).isoformat(),
    }
    if as_str:
        body["quasi:contributor"] = parse_contributor(as_str)

    result = post(f"{board}{INBOX_PATH}", body)
    print(f"\nClaimed {task_id}")
    print(f"Ledger entry: #{result.get('ledger_entry')}")
    print(f"Entry hash:   {result.get('entry_hash', '')[:16]}...")
    if as_str:
        contrib = parse_contributor(as_str)
        display = contrib.get("name") or contrib.get("handle", "")
        print(f"Attribution:  {display} — permanently anchored in the ledger")
    print()
    print("Next: implement the task, open a PR with this commit footer:")
    print()
    print(f"  Contribution-Agent: {agent}")
    print(f"  Task: {task_id}")
    print("  Verification: ci-pass")
    print()


def cmd_complete(board: str, task_id: str, agent: str, commit: str, pr: str, as_str: str | None = None) -> None:
    """Record task completion on the quasi-ledger.

    Args:
        board (str): The quasi-board URL to post to.
        task_id (str): The completed task ID.
        agent (str): The agent identifier that completed the task.
        commit (str): Merge or completion commit hash.
        pr (str): Pull request URL associated with the work.
        as_str (str | None): Optional contributor attribution string.

    Returns:
        None: Prints completion confirmation and verification guidance.
    """
    body: dict = {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "quasi:type": "completion",
        "actor": agent,
        "quasi:taskId": task_id,
        "quasi:commitHash": commit,
        "quasi:prUrl": pr,
        "published": datetime.now(timezone.utc).isoformat(),
    }
    if as_str:
        body["quasi:contributor"] = parse_contributor(as_str)

    result = post(f"{board}{INBOX_PATH}", body)
    print(f"\nCompletion recorded for {task_id}")
    print(f"Ledger entry: #{result.get('ledger_entry')}")
    print(f"Entry hash:   {result.get('entry_hash', '')[:16]}...")
    if as_str:
        contrib = parse_contributor(as_str)
        display = contrib.get("name") or contrib.get("handle", "")
        print(f"Attribution:  {display} — permanently anchored in the ledger ✓")
    print("\nYour contribution is on the quasi-ledger.")
    print(f"Verify: {board}{LEDGER_PATH}/verify")
    print()


def cmd_ledger(board: str) -> None:
    """Display the current state of the quasi-ledger.

    Args:
        board (str): The quasi-board URL to query.

    Returns:
        None: Prints the ledger to stdout.

    Side effects:
        - Makes network requests to the quasi-board ledger.
        - Prints ledger information to stdout.
        - Prints error messages to stderr on failure.
    """
    data = get(f"{board}{LEDGER_PATH}")
    chain = data.get("chain", [])
    valid = data.get("quasi:valid", False)
    print(f"\nquasi-ledger @ {board}")
    print(f"Entries:          {data.get('quasi:entries', 0)}")
    print(f"Chain valid:      {'✓' if valid else '✗ INVALID'}")
    print(f"Genesis slots:    {data.get('quasi:slotsRemaining', '?')}/50 remaining")
    print()
    if chain:
        print("Recent entries:")
        for entry in chain[-5:]:
            agent_short = entry.get('contributor_agent', '?')[:30]
            print(f"  #{entry['id']}  {entry.get('type', '?'):10}  "
                  f"{entry.get('task', '?'):12}  {agent_short}")
            print(f"       {entry['entry_hash'][:32]}...")
    else:
        print("  (no entries yet — be the first)")
    print()


def cmd_submit(board: str, task_id: str, agent: str, directory: str) -> None:
    """Submit implementation files so quasi-board can open a PR.

    Args:
        board (str): The quasi-board base URL.
        task_id (str): Claimed task ID associated with the patch.
        agent (str): Agent identifier recorded in the submission.
        directory (str): Source directory that contains the implementation files.

    Returns:
        None: Prints submission details and the PR URL opened by quasi-board.
    """
    from pathlib import Path as _Path

    base = _Path(directory).resolve()
    if not base.exists():
        print(f"Directory not found: {directory}")
        sys.exit(1)

    SKIP_DIRS = {".git", "__pycache__", "node_modules", ".venv", "dist", "build", ".next"}

    files = {}
    for f in base.rglob("*"):
        if not f.is_file():
            continue
        parts = set(f.relative_to(base).parts)
        if parts & SKIP_DIRS or any(p.startswith(".") for p in f.relative_to(base).parts):
            continue
        rel = str(f.relative_to(base))
        try:
            files[rel] = f.read_text(encoding="utf-8", errors="replace")
        except Exception:
            pass  # skip binary files

    if not files:
        print(f"No text files found in {directory}")
        sys.exit(1)

    print(f"Submitting {len(files)} file(s) for {task_id} via {board} ...")
    for path in sorted(files):
        print(f"  {path}")
    print()

    result = post(f"{board}{INBOX_PATH}", {
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            {"quasi": "https://quasi.dev/ns#"},
        ],
        "type": "Create",
        "quasi:type": "patch",
        "actor": agent,
        "quasi:taskId": task_id,
        "quasi:files": files,
        "quasi:message": f"feat: {task_id} — submitted by {agent}",
        "published": datetime.now(timezone.utc).isoformat(),
    })

    pr_url = result.get("pr_url", "")
    print(f"PR opened:    {pr_url}")
    print(f"Ledger entry: #{result.get('ledger_entry')}")
    print(f"Entry hash:   {result.get('entry_hash', '')[:16]}...")
    print()
    print("The board opened the PR on your behalf. No GitHub account needed.")
    print()


def cmd_refresh(board: str, task_id: str, agent: str) -> None:
    """Refresh the TTL on an active claim to prevent expiry during long implementations."""
    result = post(f"{board}{INBOX_PATH}", {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "quasi:Refresh",
        "actor": agent,
        "quasi:taskId": task_id,
        "published": datetime.now(timezone.utc).isoformat(),
    })
    print(f"\nClaim refreshed for {task_id}")
    print(f"New expiry:    {result.get('quasi:expiresAt', '?')}")
    print(f"Ledger entry: #{result.get('ledger_entry')}")
    print(f"Entry hash:   {result.get('entry_hash', '')[:16]}...")
    print()


def cmd_contributors(board: str) -> None:
    """List contributors from the quasi-ledger.

    Args:
        board (str): The quasi-board URL to query.

    Returns:
        None: Prints contributors to stdout.

    Side effects:
        - Makes network requests to the quasi-board ledger.
        - Prints contributor information to stdout.
        - Prints error messages to stderr on failure.
    """
    data = get(f"{board}/quasi-board/contributors")
    items = data.get("items", [])
    total = data.get("quasi:namedContributors", len(items))
    slots = data.get("quasi:genesisSlots", 50)
    print(f"\nquasi-board contributors — {total} named, {slots - total} genesis slots remaining\n")
    if not items:
        print("  No named contributors yet — be the first!")
        print("  quasi-agent claim QUASI-XXX --as \"Your Name <@handle@instance.social>\"")
    for c in items:
        badge = " [GENESIS]" if c.get("genesis") else ""
        name = c.get("name", "")
        handle = c.get("handle", "")
        display = f"{name} <{handle}>" if name and handle else name or handle
        task = c.get("task", "?")
        since = c.get("first_contribution", "")[:10]
        print(f"  {display}{badge}")
        print(f"    first contribution: {task} on {since}")
    print()


SEEN_TASKS_FILE = Path("~/.quasi/seen_tasks.json").expanduser()


def _get_quiet(url: str) -> dict | None:
    """Like get() but returns None on error instead of exiting."""
    req = urllib.request.Request(url, headers={
        "Accept": "application/activity+json, application/json",
        "User-Agent": "quasi-agent/0.1",
    })
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except Exception:
        return None


def _extract_task_info(item: dict) -> dict | None:
    """Extract task_id, title, url, status from an outbox item."""
    t = item.get("object", item) if item.get("type") == "Create" else item
    task_id = t.get("quasi:taskId")
    if not task_id:
        return None
    title = t.get("name", "")
    if not title:
        content = t.get("content", "")
        m = re.search(r"<strong>(.+?)</strong>", content)
        title = m.group(1) if m else "(no title)"
    return {
        "task_id": task_id,
        "title": title,
        "url": t.get("url", ""),
        "status": t.get("quasi:status", "open"),
    }


def _load_seen() -> set[str]:
    """Load the set of task IDs already seen by the watch command.

    Returns:
        set[str]: Previously seen task IDs, or an empty set if the cache
        file does not exist or cannot be parsed.
    """
    if SEEN_TASKS_FILE.exists():
        try:
            return set(json.loads(SEEN_TASKS_FILE.read_text()))
        except Exception:
            pass
    return set()


def _save_seen(seen: set[str]) -> None:
    """Persist the set of task IDs seen by the watch command.

    Args:
        seen (set[str]): Task IDs that should be stored in the local cache.
    """
    SEEN_TASKS_FILE.parent.mkdir(parents=True, exist_ok=True)
    SEEN_TASKS_FILE.write_text(json.dumps(sorted(seen)))


def cmd_watch(board: str, interval: int, once: bool) -> None:
    """Poll the quasi-board for newly opened tasks and print notifications.

    Args:
        board (str): The quasi-board URL to poll.
        interval (int): Poll interval in seconds.
        once (bool): If True, run one poll cycle and exit.
    """
    seen = _load_seen()
    first_run = True

    try:
        while True:
            outbox = _get_quiet(f"{board}{OUTBOX_PATH}")
            if outbox is None:
                if first_run:
                    print(f"Could not reach {board} — retrying in {interval}s")
                if once:
                    sys.exit(1)
                time.sleep(interval)
                first_run = False
                continue

            tasks = outbox.get("orderedItems", [])
            new_tasks = []
            for item in tasks:
                info = _extract_task_info(item)
                if info and info["status"] == "open" and info["task_id"] not in seen:
                    new_tasks.append(info)

            now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
            if new_tasks:
                for t in new_tasks:
                    print(f"[{now}] NEW TASK: {t['task_id']} — {t['title']}")
                    print(f"  Claim: python3 quasi-agent/cli.py --agent <model> claim {t['task_id']}")
                    seen.add(t["task_id"])
                _save_seen(seen)
            elif first_run:
                print(f"[{now}] Watching {board} — no new tasks (polling every {interval}s)")

            if once:
                if not new_tasks:
                    print(f"[{now}] No new open tasks.")
                return

            first_run = False
            time.sleep(interval)
    except KeyboardInterrupt:
        print("\nStopped watching.")


def cmd_verify(board: str) -> None:
    """Verify the integrity of the quasi-ledger.

    Args:
        board (str): The quasi-board URL to query.

    Returns:
        None: Prints verification result to stdout.

    Side effects:
        - Makes network requests to the quasi-board ledger.
        - Prints verification information to stdout.
        - Prints error messages to stderr on failure.
    """
    result = get(f"{board}{LEDGER_PATH}/verify")
    valid = result.get("valid", False)
    entries = result.get("entries", 0)
    if valid:
        print(f"✓ Ledger valid — {entries} entries, chain intact")
    else:
        print("✗ Ledger INVALID — chain broken at some entry")
    sys.exit(0 if valid else 1)


def cmd_completion(shell: str) -> None:
    """Print shell completion script to stdout."""
    commands = "list claim complete submit watch ledger contributors verify completion"
    if shell == "bash":
        print(f'''_quasi_agent() {{
    local cur prev commands
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"
    commands="{commands}"

    case "$prev" in
        claim|complete|submit)
            COMPREPLY=( $(compgen -W "--board --agent --as" -- "$cur") )
            return 0
            ;;
        completion)
            COMPREPLY=( $(compgen -W "bash zsh" -- "$cur") )
            return 0
            ;;
        --board|--agent|--as|--commit|--pr|--message|--interval)
            return 0
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--board --agent --as --interval --once --commit --pr --message" -- "$cur") )
    else
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    fi
}}
complete -F _quasi_agent quasi-agent
complete -F _quasi_agent cli.py''')
    elif shell == "zsh":
        print('''#compdef quasi-agent cli.py

_quasi_agent() {{
    local -a commands
    commands=(
        'list:List open tasks'
        'claim:Claim a task'
        'complete:Record task completion on ledger'
        'submit:Submit implementation files'
        'watch:Poll for new tasks and notify'
        'ledger:Show the ledger'
        'contributors:List named contributors'
        'verify:Verify ledger chain integrity'
        'completion:Generate shell completion script'
    )

    _arguments -C \\
        '--board[Board URL]:url:' \\
        '--agent[Agent identifier]:agent:' \\
        '--as[Attribution (name or handle)]:attribution:' \\
        '1:command:->cmd' \\
        '*::arg:->args'

    case "$state" in
        cmd)
            _describe 'command' commands
            ;;
        args)
            case "$words[1]" in
                claim)
                    _arguments '1:task-id:' '--board[Board URL]:' '--agent[Agent]:' '--as[Attribution]:'
                    ;;
                complete)
                    _arguments '1:task-id:' '--commit[Commit SHA]:' '--pr[PR URL]:' \
                        '--board[Board URL]:' '--agent[Agent]:' '--as[Attribution]:'
                    ;;
                watch)
                    _arguments '--interval[Poll interval in seconds]:' '--once[Run once and exit]'
                    ;;
                completion)
                    _arguments '1:shell:(bash zsh)'
                    ;;
            esac
            ;;
    esac
}}

compdef _quasi_agent quasi-agent
compdef _quasi_agent cli.py''')


def main() -> None:
    """Parse CLI arguments and dispatch to the selected quasi-agent command."""
    formatter = argparse.RawDescriptionHelpFormatter
    parser = argparse.ArgumentParser(
        prog="quasi-agent",
        description=(
            "quasi-agent — QUASI task client\n\n"
            "Claim tasks, refresh long-running work, and record completions on the ledger."
        ),
        formatter_class=formatter,
        epilog=textwrap.dedent(
            """\
            Common workflows:
              quasi-agent list
              quasi-agent --agent gpt-5-codex claim QUASI-001
              quasi-agent --agent gpt-5-codex refresh QUASI-001
              quasi-agent complete QUASI-001 --commit abc123 --pr https://github.com/org/repo/pull/1
              quasi-agent submit QUASI-001 --dir ./implementation

            Use `quasi-agent <command> --help` for command-specific examples.
            """
        ),
    )
    parser.add_argument("--board", default=DEFAULT_BOARD, help="quasi-board URL")
    parser.add_argument("--agent", default="quasi-agent/0.1", help="Agent identifier (model name)")
    sub = parser.add_subparsers(dest="cmd")

    p_list = sub.add_parser(
        "list",
        help="List open tasks from quasi-board",
        description="List currently open tasks from the configured quasi-board.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent list --json",
    )
    p_list.add_argument("--json", dest="output_json", action="store_true",
                        help="Output as JSON (machine-readable, useful in CI pipelines)")

    p_claim = sub.add_parser(
        "claim",
        help="Claim a task by task ID",
        description="Claim a task before starting implementation work.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent --agent gpt-5-codex claim QUASI-001",
    )
    p_claim.add_argument("task_id", help="e.g. QUASI-001")
    p_claim.add_argument(
        "--as", dest="as_str", metavar="'Name <handle>'",
        help="Optional attribution — e.g. 'Alice <@alice@fosstodon.org>'. "
             "Permanently anchored in the quasi-ledger. Always optional.",
    )

    p_complete = sub.add_parser(
        "complete",
        help="Record task completion on the quasi-ledger",
        description="Write a completion entry to the quasi-ledger after your PR is ready.",
        formatter_class=formatter,
        epilog=(
            "Example:\n"
            "  quasi-agent complete QUASI-001 --commit abc123 --pr https://github.com/org/repo/pull/1"
        ),
    )
    p_complete.add_argument("task_id", help="e.g. QUASI-001")
    p_complete.add_argument("--commit", required=True, help="Git commit hash")
    p_complete.add_argument("--pr", required=True, help="PR URL")
    p_complete.add_argument(
        "--as", dest="as_str", metavar="'Name <handle>'",
        help="Optional attribution. Permanently anchored in the quasi-ledger.",
    )

    p_refresh = sub.add_parser(
        "refresh",
        help="Refresh an active claim TTL during long-running work",
        description="Extend an active claim before it expires while work is in progress.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent --agent gpt-5-codex refresh QUASI-001",
    )
    p_refresh.add_argument("task_id", help="e.g. QUASI-001")

    p_submit = sub.add_parser(
        "submit",
        help="Submit implementation; quasi-board opens a PR on your behalf",
        description="Upload an implementation directory so the board can open a PR on your behalf.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent submit QUASI-001 --dir ./implementation",
    )
    p_submit.add_argument("task_id", help="e.g. QUASI-003")
    p_submit.add_argument("--dir", required=True, help="Directory containing your implementation")

    p_watch = sub.add_parser(
        "watch",
        help="Poll for new tasks and print notifications",
        description="Poll the board for newly opened tasks and print claim hints.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent watch --interval 120 --once",
    )
    p_watch.add_argument("--interval", type=int, default=300, help="Poll interval in seconds (default: 300)")
    p_watch.add_argument("--once", action="store_true", help="Print current open tasks and exit")

    sub.add_parser(
        "ledger",
        help="Show the current quasi-ledger state",
        description="Print the current ledger chain as returned by the board.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent ledger",
    )
    sub.add_parser(
        "contributors",
        help="List named contributors recorded in the ledger",
        description="Summarize attributed contributors found in the ledger.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent contributors",
    )
    sub.add_parser(
        "verify",
        help="Verify quasi-ledger chain integrity",
        description="Verify the local integrity of the ledger hash chain returned by the board.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent verify",
    )

    p_completion = sub.add_parser(
        "completion",
        help="Generate a shell completion script",
        description="Emit a shell completion script for bash or zsh.",
        formatter_class=formatter,
        epilog="Example:\n  quasi-agent completion zsh",
    )
    p_completion.add_argument("shell", choices=["bash", "zsh"], help="Target shell (bash or zsh)")

    argcomplete.autocomplete(parser)
    args = parser.parse_args()

    if not args.cmd:
        parser.print_help()
        return

    board = args.board.rstrip("/")

    if args.cmd == "list":
        cmd_list(board, output_json=getattr(args, "output_json", False))
    elif args.cmd == "claim":
        cmd_claim(board, args.task_id, args.agent, getattr(args, "as_str", None))
    elif args.cmd == "complete":
        cmd_complete(board, args.task_id, args.agent, args.commit, args.pr, getattr(args, "as_str", None))
    elif args.cmd == "refresh":
        cmd_refresh(board, args.task_id, args.agent)
    elif args.cmd == "submit":
        cmd_submit(board, args.task_id, args.agent, args.dir)
    elif args.cmd == "watch":
        cmd_watch(board, args.interval, args.once)
    elif args.cmd == "ledger":
        cmd_ledger(board)
    elif args.cmd == "contributors":
        cmd_contributors(board)
    elif args.cmd == "verify":
        cmd_verify(board)
    elif args.cmd == "completion":
        cmd_completion(args.shell)


if __name__ == "__main__":
    main()
