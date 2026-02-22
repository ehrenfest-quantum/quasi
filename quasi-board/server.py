#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Valiant Quantum (Daniel Hinderink)
"""
quasi-board — QUASI ActivityPub task server
The federated task feed for the QUASI Quantum OS project.

Actor: quasi-board@gawain.valiant-quantum.com
Outbox: https://gawain.valiant-quantum.com/quasi-board/outbox
Ledger: https://gawain.valiant-quantum.com/quasi-board/ledger
"""

import base64
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
OPENAPI_SPEC = Path(__file__).parent / "spec" / "openapi.json"
GITHUB_REPO = "ehrenfest-quantum/quasi"
GITHUB_TOKEN_FILE = Path("/home/vops/quasi-board/.github_token")
MATRIX_CREDS_FILE = Path("/home/openclaw/.openclaw/credentials/matrix/credentials.json")
MATRIX_ROOM_ID = "!CerauaaS111HsAzJXI:gawain.valiant-quantum.com"

AP_CONTENT_TYPE = "application/activity+json"


# ── Matrix notification ───────────────────────────────────────────────────────

async def _notify_daniel(message: str) -> None:
    """Fire-and-forget Matrix message to Daniel via Gawain homeserver."""
    try:
        if not MATRIX_CREDS_FILE.exists():
            return
        creds = json.loads(MATRIX_CREDS_FILE.read_text())
        homeserver = creds["homeserver"]
        token = creds["accessToken"]
        txn_id = f"quasi-board-{int(time.time() * 1000)}"
        room = MATRIX_ROOM_ID.replace("!", "%21").replace(":", "%3A")
        url = f"{homeserver}/_matrix/client/v3/rooms/{room}/send/m.room.message/{txn_id}"
        async with httpx.AsyncClient(timeout=5) as client:
            await client.put(
                url,
                headers={"Authorization": f"Bearer {token}"},
                json={"msgtype": "m.text", "body": message},
            )
    except Exception:
        pass  # never block the main request

# ── Submission security limits ────────────────────────────────────────────────

MAX_FILES = 50
MAX_FILE_BYTES = 100_000          # 100 KB per file
MAX_TOTAL_BYTES = 500_000         # 500 KB total payload
MAX_PATH_LEN = 200

# Paths that can never be written by agent submissions
_BLOCKED_PREFIXES = (
    ".github/",       # CI/CD workflows, CODEOWNERS, Actions secrets
    "quasi-board/",   # board server itself
    "quasi-agent/",   # agent CLI itself
    "quasi-mcp/",     # MCP server
    "infra/",         # infrastructure configs
    "spec/",          # core specification
    ".git/",          # git internals (GitHub API would reject, but belt-and-suspenders)
)

_BLOCKED_EXACT = {
    "CLAUDE.md", "README.md", "CONTRIBUTING.md", "ARCHITECTURE.md",
    "GENESIS.md", "LICENSE", ".gitignore",
}


def _validate_submission_files(files: dict) -> None:
    """Raise HTTPException if any file path or content is unsafe."""
    if not isinstance(files, dict) or not files:
        raise HTTPException(400, "quasi:files must be a non-empty dict")

    if len(files) > MAX_FILES:
        raise HTTPException(400, f"Too many files: {len(files)} > {MAX_FILES}")

    total_bytes = 0
    for path, content in files.items():
        # --- path checks ---
        if not isinstance(path, str) or not path:
            raise HTTPException(400, "File path must be a non-empty string")
        if len(path) > MAX_PATH_LEN:
            raise HTTPException(400, f"Path too long: {path[:60]}…")

        # Normalise: strip leading slashes, resolve .. sequences
        normalised = "/".join(
            p for p in path.replace("\\", "/").split("/")
            if p not in ("", ".")
        )
        # After stripping . entries, rebuild and detect traversal
        parts = normalised.split("/")
        resolved: list[str] = []
        for part in parts:
            if part == "..":
                raise HTTPException(400, f"Path traversal rejected: {path!r}")
            resolved.append(part)
        clean_path = "/".join(resolved)

        if clean_path in _BLOCKED_EXACT:
            raise HTTPException(400, f"Cannot overwrite protected file: {clean_path!r}")

        for prefix in _BLOCKED_PREFIXES:
            if clean_path.startswith(prefix) or clean_path == prefix.rstrip("/"):
                raise HTTPException(400, f"Cannot write to protected path: {clean_path!r}")

        # Replace original key with cleaned path to prevent sneaky encodings
        # (caller must use the sanitised dict we return — see _sanitise_files)

        # --- content checks ---
        if not isinstance(content, str):
            raise HTTPException(400, f"File content must be a string: {path!r}")
        file_bytes = len(content.encode("utf-8", errors="replace"))
        if file_bytes > MAX_FILE_BYTES:
            raise HTTPException(400, f"File too large ({file_bytes} bytes): {path!r}")
        total_bytes += file_bytes

    if total_bytes > MAX_TOTAL_BYTES:
        raise HTTPException(400, f"Total submission too large: {total_bytes} bytes > {MAX_TOTAL_BYTES}")


def _sanitise_files(files: dict) -> dict:
    """Return a new dict with normalised, safe paths."""
    out = {}
    for path, content in files.items():
        clean = "/".join(
            p for p in path.replace("\\", "/").split("/")
            if p not in ("", ".", "..")
        )
        out[clean] = content
    return out


def _validate_task_id(task_id: str) -> None:
    import re
    if not re.fullmatch(r"QUASI-\d{1,6}", task_id):
        raise HTTPException(400, f"Invalid task_id format: {task_id!r} — expected QUASI-NNN")


def _check_agent_claimed(task_id: str, agent: str) -> None:
    """Reject submission if this agent has no claim entry for this task."""
    chain = load_ledger()
    for entry in chain:
        if (
            entry.get("type") == "claim"
            and entry.get("task") == task_id
            and entry.get("contributor_agent") == agent
        ):
            return
    raise HTTPException(403, f"Agent {agent!r} has not claimed {task_id} — call claim first")


# ── GitHub PR helper ──────────────────────────────────────────────────────────

def _github_token() -> str:
    if GITHUB_TOKEN_FILE.exists():
        return GITHUB_TOKEN_FILE.read_text().strip()
    return os.environ.get("QUASI_GITHUB_TOKEN", "")


async def _open_pr_from_files(task_id: str, agent: str, files: dict, message: str) -> str:
    """Create a branch with agent-supplied files and open a PR. Returns PR URL."""
    token = _github_token()
    if not token:
        raise HTTPException(500, "quasi-board: no GitHub token configured")

    headers = {
        "Authorization": f"Bearer {token}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }

    # Branch name: only alphanumeric + hyphen/slash, agent truncated, no injections
    safe_agent = "".join(c if c.isalnum() or c == "-" else "-" for c in agent)[:24].strip("-")
    branch = f"agent/{task_id.lower()}-{safe_agent}"

    # Sanitize message: strip newlines to prevent header injection in PR body
    safe_message = (message or "")[:500].replace("\r", " ").replace("\n", " ")

    async with httpx.AsyncClient(timeout=30) as gh:
        # Get main SHA
        r = await gh.get(
            f"https://api.github.com/repos/{GITHUB_REPO}/git/ref/heads/main",
            headers=headers,
        )
        r.raise_for_status()
        main_sha = r.json()["object"]["sha"]

        # Create branch (ignore 422 = already exists)
        r = await gh.post(
            f"https://api.github.com/repos/{GITHUB_REPO}/git/refs",
            headers=headers,
            json={"ref": f"refs/heads/{branch}", "sha": main_sha},
        )
        if r.status_code not in (201, 422):
            r.raise_for_status()

        # Create/update each file
        for path, content in files.items():
            encoded = base64.b64encode(content.encode("utf-8", errors="replace")).decode()
            # Get current file SHA if it exists on this branch (needed for update)
            existing = await gh.get(
                f"https://api.github.com/repos/{GITHUB_REPO}/contents/{path}",
                headers=headers,
                params={"ref": branch},
            )
            payload: dict = {
                "message": f"feat: {task_id}",
                "content": encoded,
                "branch": branch,
            }
            if existing.status_code == 200:
                payload["sha"] = existing.json()["sha"]
            r = await gh.put(
                f"https://api.github.com/repos/{GITHUB_REPO}/contents/{path}",
                headers=headers,
                json=payload,
            )
            r.raise_for_status()

        # Open PR — metadata is board-generated, not agent-controlled
        pr_body = (
            f"## {task_id}\n\n"
            f"_{safe_message}_\n\n"
            f"---\n"
            f"Contribution-Agent: `{agent}`\n"
            f"Task: `{task_id}`\n"
            f"Submitted-Via: quasi-board patch submission\n"
        )
        r = await gh.post(
            f"https://api.github.com/repos/{GITHUB_REPO}/pulls",
            headers=headers,
            json={
                "title": f"feat: {task_id}",
                "body": pr_body,
                "head": branch,
                "base": "main",
            },
        )
        if r.status_code == 422:
            existing_prs = await gh.get(
                f"https://api.github.com/repos/{GITHUB_REPO}/pulls",
                headers=headers,
                params={"head": f"ehrenfest-quantum:{branch}", "state": "open"},
            )
            if existing_prs.status_code == 200 and existing_prs.json():
                return existing_prs.json()[0]["html_url"]
        r.raise_for_status()
        return r.json()["html_url"]

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
        "quasi:moltbook": "daniel@arvak.io",
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
        await _notify_daniel(
            f"🤖 QUASI: {agent} claimed {task_id} — ledger #{entry['id']}"
        )
        return JSONResponse({"status": "claimed", "ledger_entry": entry["id"], "entry_hash": entry["entry_hash"]})

    if activity_type == "Create" and body.get("quasi:type") == "patch":
        # Agent submitting implementation — board opens PR on their behalf
        task_id = body.get("quasi:taskId", "")
        agent = body.get("actor", "unknown")
        files = body.get("quasi:files", {})
        message = body.get("quasi:message", "")

        if not task_id:
            raise HTTPException(400, "quasi:taskId required")

        # Security: validate task_id format, file paths, sizes, and claim
        _validate_task_id(task_id)
        _validate_submission_files(files)
        _check_agent_claimed(task_id, agent)
        files = _sanitise_files(files)

        pr_url = await _open_pr_from_files(task_id, agent, files, message)

        entry = append_ledger({
            "type": "submission",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": pr_url,
        })
        await _notify_daniel(
            f"🤖 QUASI: {agent} submitted {task_id} — PR opened: {pr_url} — ledger #{entry['id']}"
        )
        return JSONResponse({
            "status": "pr_opened",
            "pr_url": pr_url,
            "ledger_entry": entry["id"],
            "entry_hash": entry["entry_hash"],
        })

    if activity_type == "Create" and body.get("quasi:type") == "completion":
        # Agent reporting a completed task (manual flow)
        agent = body.get("actor", "unknown")
        task_id = body.get("quasi:taskId", "")
        entry = append_ledger({
            "type": "completion",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": body.get("quasi:commitHash"),
            "pr_url": body.get("quasi:prUrl"),
        })
        await _notify_daniel(
            f"✅ QUASI: {agent} completed {task_id} — ledger #{entry['id']}"
        )
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
