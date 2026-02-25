# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Daniel Hinderink
"""
quasi-agent — QUASI task client

Connects to any quasi-board ActivityPub instance.
Lists open tasks, claims them, records completions on the ledger.
"""

import json
import re
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

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


def list_tasks(board: str = DEFAULT_BOARD) -> list:
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
        })
    return parsed


def claim_task(task_id: str, board: str = DEFAULT_BOARD, as_str: str = "") -> dict:
    contributor = parse_contributor(as_str)
    body = {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Accept",
        "actor": {
            "type": "Service",
            "name": "quasi-agent",
            "url": "https://github.com/ehrenfest-quantum/quasi"
        },
        "object": {
            "type": "Task",
            "quasi:taskId": task_id,
            "quasi:status": "claimed",
            "quasi:contributor": contributor
        }
    }
    return post(f"{board}{INBOX_PATH}", body)


def complete_task(task_id: str, board: str = DEFAULT_BOARD, commit: str = "", pr: str = "", as_str: str = "") -> dict:
    contributor = parse_contributor(as_str)
    body = {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Update",
        "actor": {
            "type": "Service",
            "name": "quasi-agent",
            "url": "https://github.com/ehrenfest-quantum/quasi"
        },
        "object": {
            "type": "Task",
            "quasi:taskId": task_id,
            "quasi:status": "completed",
            "quasi:contributor": contributor,
            "quasi:commit": commit,
            "quasi:pr": pr
        }
    }
    return post(f"{board}{INBOX_PATH}", body)
