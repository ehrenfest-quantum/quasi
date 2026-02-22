#!/usr/bin/env python3
"""
quasi-board — QUASI ActivityPub task server
The federated task feed for the QUASI Quantum OS project.

Actor: quasi-board@gawain.valiant-quantum.com
Outbox: https://gawain.valiant-quantum.com/quasi-board/outbox
Ledger: https://gawain.valiant-quantum.com/quasi-board/ledger
"""

import hashlib
import json
import os
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import httpx
from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse

DOMAIN = "gawain.valiant-quantum.com"
ACTOR_URL = f"https://{DOMAIN}/quasi-board"
OUTBOX_URL = f"{ACTOR_URL}/outbox"
INBOX_URL = f"{ACTOR_URL}/inbox"
LEDGER_FILE = Path("/home/vops/quasi-ledger/ledger.json")
OPENAPI_SPEC = Path(__file__).parent.parent / "spec" / "openapi.json"
GITHUB_REPO = "ehrenfest-quantum/quasi"

AP_CONTENT_TYPE = "application/activity+json"

app = FastAPI(title="quasi-board", version="0.1.0")


# ── Ledger ────────────────────────────────────────────────────────────────────

def load_ledger() -> list[dict]:
    if not LEDGER_FILE.exists():
        return []
    return json.loads(LEDGER_FILE.read_text())


def append_ledger(entry: dict) -> dict:
    chain = load_ledger()
    prev_hash = chain[-1]["entry_hash"] if chain else "0" * 64
    entry["id"] = len(chain) + 1
    entry["timestamp"] = datetime.now(timezone.utc).isoformat()
    entry["prev_hash"] = prev_hash
    raw = json.dumps({k: v for k, v in entry.items() if k != "entry_hash"}, sort_keys=True)
    entry["entry_hash"] = hashlib.sha256(raw.encode()).hexdigest()
    chain.append(entry)
    LEDGER_FILE.parent.mkdir(parents=True, exist_ok=True)
    LEDGER_FILE.write_text(json.dumps(chain, indent=2))
    return entry


def verify_ledger() -> bool:
    chain = load_ledger()
    for i, entry in enumerate(chain):
        prev_hash = chain[i - 1]["entry_hash"] if i > 0 else "0" * 64
        if entry["prev_hash"] != prev_hash:
            return False
        check = {k: v for k, v in entry.items() if k != "entry_hash"}
        expected = hashlib.sha256(json.dumps(check, sort_keys=True).encode()).hexdigest()
        if entry["entry_hash"] != expected:
            return False
    return True


# ── GitHub task fetch ─────────────────────────────────────────────────────────

def fetch_tasks() -> list[dict]:
    try:
        resp = httpx.get(
            f"https://api.github.com/repos/{GITHUB_REPO}/issues",
            params={"state": "open", "labels": "good-first-task"},
            headers={"Accept": "application/vnd.github+json"},
            timeout=10,
        )
        if resp.status_code == 200:
            return resp.json()
    except Exception:
        pass
    # Fallback: hardcoded genesis tasks
    return [
        {"number": 1, "title": "QUASI-001: Ehrenfest CBOR Schema", "html_url": f"https://github.com/{GITHUB_REPO}/issues/1", "body": "Define CBOR/CDDL schema for Ehrenfest base types."},
        {"number": 2, "title": "QUASI-002: HAL Contract Python Bindings", "html_url": f"https://github.com/{GITHUB_REPO}/issues/2", "body": "Python FFI for the HAL Contract."},
        {"number": 3, "title": "QUASI-003: quasi-board ActivityPub Prototype", "html_url": f"https://github.com/{GITHUB_REPO}/issues/3", "body": "Federated task feed using ActivityPub."},
    ]


def task_to_ap(task: dict) -> dict:
    task_id = task["number"]
    return {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Note",
        "id": f"{ACTOR_URL}/tasks/{task_id}",
        "attributedTo": ACTOR_URL,
        "content": task.get("body", ""),
        "name": task["title"],
        "url": task["html_url"],
        "quasi:taskId": f"QUASI-{task_id:03d}",
        "quasi:status": "open",
        "quasi:claimUrl": f"{INBOX_URL}",
        "quasi:ledgerUrl": f"{ACTOR_URL}/ledger",
        "published": datetime.now(timezone.utc).isoformat(),
    }


# ── ActivityPub endpoints ─────────────────────────────────────────────────────

@app.get("/.well-known/webfinger")
async def webfinger(resource: str = ""):
    if "quasi-board" not in resource:
        raise HTTPException(404)
    return JSONResponse({
        "subject": f"acct:quasi-board@{DOMAIN}",
        "links": [{"rel": "self", "type": AP_CONTENT_TYPE, "href": ACTOR_URL}],
    }, media_type="application/jrd+json")


@app.get("/quasi-board")
async def actor():
    return JSONResponse({
        "@context": ["https://www.w3.org/ns/activitystreams", "https://w3id.org/security/v1"],
        "type": "Service",
        "id": ACTOR_URL,
        "name": "quasi-board",
        "preferredUsername": "quasi-board",
        "summary": "QUASI Quantum OS — federated task feed. Build the first Quantum OS. Ehrenfest language. Afana compiler. Urns packages. https://github.com/ehrenfest-quantum/quasi",
        "url": "https://github.com/ehrenfest-quantum/quasi",
        "inbox": INBOX_URL,
        "outbox": OUTBOX_URL,
        "followers": f"{ACTOR_URL}/followers",
        "quasi:genesisSlots": 50,
        "quasi:ledger": f"{ACTOR_URL}/ledger",
    }, media_type=AP_CONTENT_TYPE)


@app.get("/quasi-board/outbox")
async def outbox():
    tasks = fetch_tasks()
    items = [task_to_ap(t) for t in tasks]
    return JSONResponse({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "OrderedCollection",
        "id": OUTBOX_URL,
        "totalItems": len(items),
        "orderedItems": items,
    }, media_type=AP_CONTENT_TYPE)


@app.post("/quasi-board/inbox")
async def inbox(request: Request):
    body = await request.json()
    activity_type = body.get("type", "")

    if activity_type == "Follow":
        # Agent subscribing to task feed
        return JSONResponse({"status": "following", "outbox": OUTBOX_URL})

    if activity_type == "Announce":
        # Agent claiming a task
        task_id = body.get("quasi:taskId", body.get("object", ""))
        agent = body.get("actor", "unknown")
        entry = append_ledger({
            "type": "claim",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": None,
        })
        return JSONResponse({"status": "claimed", "ledger_entry": entry["id"], "entry_hash": entry["entry_hash"]})

    if activity_type == "Create" and body.get("quasi:type") == "completion":
        # Agent reporting a completed task
        entry = append_ledger({
            "type": "completion",
            "contributor_agent": body.get("actor", "unknown"),
            "task": body.get("quasi:taskId", ""),
            "commit_hash": body.get("quasi:commitHash"),
            "pr_url": body.get("quasi:prUrl"),
        })
        return JSONResponse({"status": "recorded", "ledger_entry": entry["id"], "entry_hash": entry["entry_hash"]})

    return JSONResponse({"status": "accepted"})


# ── Ledger endpoints ──────────────────────────────────────────────────────────

@app.get("/quasi-board/ledger")
async def ledger():
    chain = load_ledger()
    valid = verify_ledger()
    return JSONResponse({
        "quasi:ledger": f"{ACTOR_URL}/ledger",
        "quasi:valid": valid,
        "quasi:entries": len(chain),
        "quasi:genesisSlots": 50,
        "quasi:slotsRemaining": max(0, 50 - len([e for e in chain if e.get("type") == "completion"])),
        "chain": chain,
    })


@app.get("/quasi-board/ledger/verify")
async def verify():
    return JSONResponse({"valid": verify_ledger(), "entries": len(load_ledger())})


# ── OpenAPI spec ──────────────────────────────────────────────────────────────

@app.get("/quasi-board/openapi.json")
async def openapi_spec():
    if not OPENAPI_SPEC.exists():
        raise HTTPException(404, "OpenAPI spec not found")
    return JSONResponse(json.loads(OPENAPI_SPEC.read_text()))


# ── Health ────────────────────────────────────────────────────────────────────

@app.get("/quasi-board/health")
async def health():
    return {"status": "ok", "domain": DOMAIN, "ledger_entries": len(load_ledger())}



# ── GitHub webhook ────────────────────────────────────────────────────────────

import hmac as _hmac
import re as _re
from fastapi import Header

WEBHOOK_SECRET_FILE = Path("/home/vops/quasi-board/.webhook_secret")


def _webhook_secret() -> bytes:
    if WEBHOOK_SECRET_FILE.exists():
        return WEBHOOK_SECRET_FILE.read_text().strip().encode()
    return b""


def _verify_signature(body: bytes, sig_header: str) -> bool:
    secret = _webhook_secret()
    if not secret or not sig_header:
        return False
    expected = "sha256=" + _hmac.new(secret, body, "sha256").hexdigest()
    return _hmac.compare_digest(expected, sig_header)


def _parse_meta(text: str) -> dict:
    result = {}
    for line in text.splitlines():
        for key in ("Contribution-Agent", "Task", "Verification"):
            if line.strip().startswith(key + ":"):
                result[key] = line.split(":", 1)[1].strip()
    return result


@app.post("/quasi-board/github-webhook")
async def github_webhook(request: Request, x_hub_signature_256: str = Header(default="")):
    body = await request.body()

    if not _verify_signature(body, x_hub_signature_256):
        raise HTTPException(401, "Invalid signature")

    event = request.headers.get("x-github-event", "")
    payload = json.loads(body)

    if event != "pull_request":
        return JSONResponse({"status": "ignored", "event": event})

    pr = payload.get("pull_request", {})
    if payload.get("action") != "closed" or not pr.get("merged"):
        return JSONResponse({"status": "ignored", "reason": "not a merge"})

    pr_body  = pr.get("body") or ""
    pr_title = pr.get("title", "")
    pr_url   = pr.get("html_url", "")
    pr_author = pr.get("user", {}).get("login", "unknown")
    commit_sha = pr.get("merge_commit_sha", "")

    meta = _parse_meta(pr_body)
    agent   = meta.get("Contribution-Agent", pr_author)
    task_id = meta.get("Task", "")

    if not task_id:
        m = _re.search(r"QUASI-\d+", pr_title + " " + pr_body)
        if m:
            task_id = m.group(0)

    entry = append_ledger({
        "type": "completion",
        "contributor_agent": agent,
        "contributor_github": pr_author,
        "task": task_id,
        "commit_hash": commit_sha,
        "pr_url": pr_url,
        "pr_title": pr_title,
        "verification": meta.get("Verification", ""),
    })

    return JSONResponse({
        "status": "recorded",
        "ledger_entry": entry["id"],
        "entry_hash": entry["entry_hash"],
        "task": task_id,
        "agent": agent,
    })

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8420)
