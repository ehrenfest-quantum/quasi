#!/usr/bin/env python3
"""Werner Shadow Collector — collects new issues without evaluating.

Polls the quasi-ledger on Camelot, records issue_generated events
into a queue file. No Werner API calls — just collection.

Evaluation happens later in batch via shadow_sealed.py --batch.

Usage:
    python shadow_collector.py                # poll continuously
    python shadow_collector.py --once         # single pass
    python shadow_collector.py --status       # show queue status

Deploy on Camelot:
    nohup python3 shadow_collector.py --interval 300 > /dev/null 2>&1 &
"""

from __future__ import annotations

import argparse
import json
import logging
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import config

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("werner-collector")

QUEUE_FILE = Path(__file__).parent / "shadow-queue.jsonl"
STATE_FILE = Path(__file__).parent / ".collector-state.json"

# Remote paths (when running on Camelot directly)
REMOTE_QUEUE = "/home/vops/werner-shadow/queue.jsonl"
REMOTE_STATE = "/home/vops/werner-shadow/.collector-state.json"


# Ledger event types that mean "a GitHub issue now exists and is judgeable".
#
#   issue_generated   - legacy A-track path, where the senate opened the issue
#                       itself. Still emitted when SENATE_DIRECT_ISSUES=1.
#   proposal_accepted - current path. The senate only *proposes*; the board
#                       opens the issue when a human accepts, and records the
#                       issue number directly rather than a URL.
ISSUE_EVENT_TYPES = ("issue_generated", "proposal_accepted")

_REPO = "ehrenfest-quantum/quasi"


def issue_ref(entry):
    """Return (issue_number, issue_url) for a ledger entry, or (None, "")."""
    num = entry.get("issue_number")
    if isinstance(num, int) and num > 0:
        return num, "https://github.com/%s/issues/%d" % (_REPO, num)
    url = entry.get("issue_url") or entry.get("url", "")
    return extract_issue_number(url), url


def load_state() -> dict[str, Any]:
    """Load collector state."""
    # Try remote first (if on Camelot), then local
    for path in [Path(REMOTE_STATE), STATE_FILE]:
        if path.exists():
            return json.loads(path.read_text())
    return {"last_seen_index": -1}


def save_state(state: dict[str, Any]) -> None:
    """Save state to both local and remote (if accessible)."""
    state_json = json.dumps(state, indent=2)
    STATE_FILE.write_text(state_json)
    # Try remote too
    try:
        Path(REMOTE_STATE).parent.mkdir(parents=True, exist_ok=True)
        Path(REMOTE_STATE).write_text(state_json)
    except OSError:
        pass


def fetch_ledger_local() -> list[dict[str, Any]]:
    """Read ledger directly (when running on Camelot)."""
    ledger_path = Path(config.LEDGER_PATH)
    if ledger_path.exists():
        return json.loads(ledger_path.read_text())
    return []


def fetch_ledger_ssh() -> list[dict[str, Any]]:
    """Fetch ledger via SSH (when running remotely)."""
    result = subprocess.run(
        ["ssh", f"root@{config.CAMELOT_HOST}", f"cat {config.LEDGER_PATH}"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        log.error("SSH fetch failed: %s", result.stderr.strip())
        return []
    return json.loads(result.stdout)


def fetch_ledger() -> list[dict[str, Any]]:
    """Fetch ledger — local if on Camelot, SSH otherwise."""
    local = fetch_ledger_local()
    if local:
        return local
    return fetch_ledger_ssh()


def extract_issue_number(url: str) -> int | None:
    """Extract issue number from GitHub URL."""
    import re
    match = re.search(r"/issues/(\d+)", url)
    return int(match.group(1)) if match else None


def append_to_queue(entry: dict[str, Any]) -> None:
    """Append an issue to the queue."""
    queue_path = QUEUE_FILE
    # Also try remote path
    for path in [queue_path, Path(REMOTE_QUEUE)]:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            with open(path, "a") as f:
                f.write(json.dumps(entry) + "\n")
        except OSError:
            pass


def load_queue() -> list[dict[str, Any]]:
    """Load all queued issues."""
    entries = []
    for path in [QUEUE_FILE, Path(REMOTE_QUEUE)]:
        if path.exists():
            for line in path.read_text().strip().split("\n"):
                if line.strip():
                    entries.append(json.loads(line))
            break
    return entries


_shutdown = False


def _handle_signal(signum: int, _frame: Any) -> None:
    global _shutdown  # noqa: PLW0603
    _shutdown = True


def run(once: bool = False, interval: int = 300) -> None:
    """Main collection loop."""
    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    state = load_state()
    last_seen = state.get("last_seen_index", -1)
    log.info("Werner Collector started. last_seen=%d, interval=%ds", last_seen, interval)

    while not _shutdown:
        try:
            ledger = fetch_ledger()
            new_entries = [
                (i, e) for i, e in enumerate(ledger)
                if i > last_seen and e.get("type") in ISSUE_EVENT_TYPES
            ]

            for idx, entry in new_entries:
                issue_number, issue_url = issue_ref(entry)
                if issue_number is None:
                    continue

                queue_entry = {
                    "collected_at": datetime.now(timezone.utc).isoformat(),
                    "ledger_index": idx,
                    "issue_number": issue_number,
                    "issue_url": issue_url,
                    "ledger_type": entry.get("type"),
                    "evaluated": False,
                }
                append_to_queue(queue_entry)
                log.info("Queued issue #%d (ledger idx %d)", issue_number, idx)

                last_seen = idx
                save_state({"last_seen_index": last_seen})

            if new_entries:
                log.info("Queued %d new issues", len(new_entries))

        except Exception:
            log.exception("Error in collection cycle")

        if once:
            break

        for _ in range(interval):
            if _shutdown:
                break
            time.sleep(1)

    log.info("Collector stopped. last_seen=%d", last_seen)


def show_status() -> None:
    """Show queue status."""
    queue = load_queue()
    state = load_state()
    evaluated = sum(1 for e in queue if e.get("evaluated"))
    pending = len(queue) - evaluated

    print(f"Queue: {len(queue)} total, {pending} pending, {evaluated} evaluated")
    print(f"Last seen ledger index: {state.get('last_seen_index', -1)}")

    if queue:
        first = queue[0]
        last = queue[-1]
        print(f"First: #{first['issue_number']} at {first['collected_at']}")
        print(f"Last:  #{last['issue_number']} at {last['collected_at']}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Werner Shadow Collector")
    parser.add_argument("--once", action="store_true", help="Single pass")
    parser.add_argument("--status", action="store_true", help="Show queue status")
    parser.add_argument("--interval", type=int, default=300, help="Poll interval (default: 5 min)")
    args = parser.parse_args()

    if args.status:
        show_status()
        return

    run(once=args.once, interval=args.interval)


if __name__ == "__main__":
    main()
