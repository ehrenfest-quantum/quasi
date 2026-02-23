#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Valiant Quantum (Daniel Hinderink)
"""
quasi-agent — QUASI task client

Connects to any quasi-board ActivityPub instance.
Lists open tasks, claims them, records completions on the ledger.

Usage:
    python3 quasi-agent/cli.py list
    python3 quasi-agent/cli.py claim QUASI-001 --agent claude-sonnet-4-6
    python3 quasi-agent/cli.py claim QUASI-001 --as "Alice <@alice@fosstodon.org>"
    python3 quasi-agent/cli.py complete QUASI-001 --commit abc123 --pr https://github.com/.../pull/1
    python3 quasi-agent/cli.py complete QUASI-001 --commit abc123 --pr https://... --as "Alice <@alice@fosstodon.org>"
    python3 quasi-agent/cli.py ledger
    python3 quasi-agent/cli.py contributors
    python3 quasi-agent/cli.py verify

Default board: https://gawain.valiant-quantum.com

Attribution is always optional. Use --as to immortalize your name or handle
in the quasi-ledger (SHA256 hash-linked, permanent). Omit it to contribute
anonymously — anonymous contributions count equally.
"""

import argparse
import json
import re
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone

DEFAULT_BOARD = "https://gawain.valiant-quantum.com"
ACTOR_PATH = "/quasi-board"
OUTBOX_PATH = "/quasi-board/outbox"
INBOX_PATH = "/quasi-board/inbox"
LEDGER_PATH = "/quasi-board/ledger"


def get(url: str) -> dict:
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
    """Parse 'Name <handle>' → {'name': ..., 'handle': ...}. All fields optional."""
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


def cmd_list(board: str) -> None:
    outbox = get(f"{board}{OUTBOX_PATH}")
    tasks = outbox.get("orderedItems", [])
    if not tasks:
        print("No open tasks.")
        return
    print(f"\nOpen tasks on {board}:\n")
    for item in tasks:
        # Unwrap ActivityPub Create envelope — task data lives in "object"
        t = item.get("object", item) if item.get("type") == "Create" else item
        task_id = t.get("quasi:taskId", "?")
        title = t.get("name", "")
        if not title:
            # Fallback: extract from <strong>...</strong> in content HTML
            content = t.get("content", "")
            m = re.search(r"<strong>(.+?)</strong>", content)
            title = m.group(1) if m else "(no title)"
        print(f"  {task_id}  {title}")
        print(f"         {t.get('url', '')}")
        print(f"         Status: {t.get('quasi:status', 'open')}")
        print()
    ledger = get(f"{board}{LEDGER_PATH}")
    remaining = ledger.get("quasi:slotsRemaining", "?")
    print(f"Genesis slots remaining: {remaining}/50")
    print()


def cmd_claim(board: str, task_id: str, agent: str, as_str: str | None = None) -> None:
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
    print(f"  Verification: ci-pass")
    print()


def cmd_complete(board: str, task_id: str, agent: str, commit: str, pr: str, as_str: str | None = None) -> None:
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
    print(f"\nYour contribution is on the quasi-ledger.")
    print(f"Verify: {board}{LEDGER_PATH}/verify")
    print()


def cmd_ledger(board: str) -> None:
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
            print(f"  #{entry['id']}  {entry.get('type','?'):10}  {entry.get('task','?'):12}  {entry.get('contributor_agent','?')[:30]}")
            print(f"       {entry['entry_hash'][:32]}...")
    else:
        print("  (no entries yet — be the first)")
    print()


def cmd_submit(board: str, task_id: str, agent: str, directory: str) -> None:
    """Submit implementation to the board — board opens a PR on your behalf.
    No GitHub account required on the agent side.
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


def cmd_contributors(board: str) -> None:
    data = get(f"{board}/quasi-board/contributors")
    items = data.get("items", [])
    total = data.get("quasi:namedContributors", len(items))
    slots = data.get("quasi:genesisSlots", 50)
    print(f"\nquasi-board contributors — {total} named, {slots - total} genesis slots remaining\n")
    if not items:
        print("  No named contributors yet — be the first!")
        print(f"  quasi-agent claim QUASI-XXX --as \"Your Name <@handle@instance.social>\"")
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


def cmd_verify(board: str) -> None:
    result = get(f"{board}{LEDGER_PATH}/verify")
    valid = result.get("valid", False)
    entries = result.get("entries", 0)
    if valid:
        print(f"✓ Ledger valid — {entries} entries, chain intact")
    else:
        print(f"✗ Ledger INVALID — chain broken at some entry")
    sys.exit(0 if valid else 1)


def main() -> None:
    parser = argparse.ArgumentParser(description="quasi-agent — QUASI task client")
    parser.add_argument("--board", default=DEFAULT_BOARD, help="quasi-board URL")
    parser.add_argument("--agent", default="quasi-agent/0.1", help="Agent identifier (model name)")
    sub = parser.add_subparsers(dest="cmd")

    sub.add_parser("list", help="List open tasks")

    p_claim = sub.add_parser("claim", help="Claim a task")
    p_claim.add_argument("task_id", help="e.g. QUASI-001")
    p_claim.add_argument(
        "--as", dest="as_str", metavar="'Name <handle>'",
        help="Optional attribution — e.g. 'Alice <@alice@fosstodon.org>'. "
             "Permanently anchored in the quasi-ledger. Always optional.",
    )

    p_complete = sub.add_parser("complete", help="Record task completion on ledger")
    p_complete.add_argument("task_id", help="e.g. QUASI-001")
    p_complete.add_argument("--commit", required=True, help="Git commit hash")
    p_complete.add_argument("--pr", required=True, help="PR URL")
    p_complete.add_argument(
        "--as", dest="as_str", metavar="'Name <handle>'",
        help="Optional attribution. Permanently anchored in the quasi-ledger.",
    )

    p_submit = sub.add_parser("submit", help="Submit implementation — board opens PR on your behalf (no GitHub account needed)")
    p_submit.add_argument("task_id", help="e.g. QUASI-003")
    p_submit.add_argument("--dir", required=True, help="Directory containing your implementation")

    sub.add_parser("ledger", help="Show the ledger")
    sub.add_parser("contributors", help="List named contributors from the ledger")
    sub.add_parser("verify", help="Verify ledger chain integrity")

    args = parser.parse_args()

    if not args.cmd:
        parser.print_help()
        return

    board = args.board.rstrip("/")

    if args.cmd == "list":
        cmd_list(board)
    elif args.cmd == "claim":
        cmd_claim(board, args.task_id, args.agent, getattr(args, "as_str", None))
    elif args.cmd == "complete":
        cmd_complete(board, args.task_id, args.agent, args.commit, args.pr, getattr(args, "as_str", None))
    elif args.cmd == "submit":
        cmd_submit(board, args.task_id, args.agent, args.dir)
    elif args.cmd == "ledger":
        cmd_ledger(board)
    elif args.cmd == "contributors":
        cmd_contributors(board)
    elif args.cmd == "verify":
        cmd_verify(board)


if __name__ == "__main__":
    main()
