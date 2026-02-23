#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
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
from datetime import datetime, timezone, timedelta
from email.utils import formatdate
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import httpx
from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import HTMLResponse, JSONResponse

DOMAIN = "gawain.valiant-quantum.com"
ACTOR_URL = f"https://{DOMAIN}/quasi-board"
OUTBOX_URL = f"{ACTOR_URL}/outbox"
INBOX_URL = f"{ACTOR_URL}/inbox"
LEDGER_FILE = Path("/home/vops/quasi-ledger/ledger.json")
OPENAPI_SPEC = Path(__file__).parent / "spec" / "openapi.json"
GITHUB_REPO = "ehrenfest-quantum/quasi"
GITHUB_TOKEN_FILE = Path("/home/vops/quasi-board/.github_token")
MATRIX_CREDS_FILE = Path("/home/vops/quasi-board/matrix_credentials.json")
MATRIX_ROOM_ID = "!CerauaaS111HsAzJXI:gawain.valiant-quantum.com"
ACTOR_KEY_FILE = Path("/home/vops/quasi-board/keys/actor.pem")
FOLLOWERS_FILE = Path("/home/vops/quasi-board/followers.json")
ACTOR_KEY_ID = f"{ACTOR_URL}#main-key"

AP_CONTENT_TYPE = "application/activity+json"
CLAIM_TTL_MINUTES = 30
CLAIM_TTL_HOURS = CLAIM_TTL_MINUTES / 60  # 0.5h — keeps timedelta math unchanged


# ── HTTP Signatures ───────────────────────────────────────────────────────────

def _load_or_create_keys():
    """Load RSA-2048 key pair from disk, generating if absent. Returns (private_key, public_key_pem)."""
    from cryptography.hazmat.primitives.asymmetric import rsa
    from cryptography.hazmat.primitives import serialization

    ACTOR_KEY_FILE.parent.mkdir(parents=True, exist_ok=True)

    if ACTOR_KEY_FILE.exists():
        from cryptography.hazmat.primitives.serialization import load_pem_private_key
        private_key = load_pem_private_key(ACTOR_KEY_FILE.read_bytes(), password=None)
    else:
        private_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        ACTOR_KEY_FILE.write_bytes(private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        ))
        ACTOR_KEY_FILE.chmod(0o600)

    public_key_pem = private_key.public_key().public_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    ).decode()

    return private_key, public_key_pem


_private_key, _public_key_pem = _load_or_create_keys()


def _make_digest(body: bytes) -> str:
    return "SHA-256=" + base64.b64encode(hashlib.sha256(body).digest()).decode()


def _sign_request(method: str, url: str, body: bytes) -> dict[str, str]:
    """Return headers dict (Date, Digest, Signature) for an outgoing ActivityPub request."""
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import padding as ap

    parsed = urlparse(url)
    host = parsed.netloc
    path = parsed.path or "/"
    date = formatdate(usegmt=True)
    digest = _make_digest(body)

    signing_string = (
        f"(request-target): {method.lower()} {path}\n"
        f"host: {host}\n"
        f"date: {date}\n"
        f"digest: {digest}"
    )

    signature_bytes = _private_key.sign(
        signing_string.encode(),
        ap.PKCS1v15(),
        hashes.SHA256(),
    )
    signature_b64 = base64.b64encode(signature_bytes).decode()

    signature_header = (
        f'keyId="{ACTOR_KEY_ID}",'
        f'algorithm="rsa-sha256",'
        f'headers="(request-target) host date digest",'
        f'signature="{signature_b64}"'
    )

    return {"Date": date, "Digest": digest, "Signature": signature_header}


async def _deliver(inbox_url: str, activity: dict) -> None:
    """POST a signed ActivityPub activity to a remote inbox. Fire-and-forget."""
    try:
        body = json.dumps(activity).encode()
        sig_headers = _sign_request("POST", inbox_url, body)
        async with httpx.AsyncClient(timeout=10) as client:
            await client.post(
                inbox_url,
                content=body,
                headers={
                    "Content-Type": AP_CONTENT_TYPE,
                    "Accept": AP_CONTENT_TYPE,
                    **sig_headers,
                },
            )
    except Exception:
        pass  # delivery is best-effort


# ── Followers ─────────────────────────────────────────────────────────────────

def _load_followers() -> list[str]:
    if not FOLLOWERS_FILE.exists():
        return []
    return json.loads(FOLLOWERS_FILE.read_text()).get("followers", [])


def _save_follower(actor_url: str) -> None:
    followers = _load_followers()
    if actor_url not in followers:
        followers.append(actor_url)
        FOLLOWERS_FILE.write_text(json.dumps({"followers": followers}, indent=2))


async def _deliver_to_followers(activity: dict) -> None:
    """Deliver an activity to all known followers' inboxes."""
    followers = _load_followers()
    for actor_url in followers:
        try:
            async with httpx.AsyncClient(timeout=5) as client:
                r = await client.get(actor_url, headers={"Accept": AP_CONTENT_TYPE})
                if r.status_code == 200:
                    inbox = r.json().get("inbox")
                    if inbox:
                        await _deliver(inbox, activity)
        except Exception:
            pass


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


def _effective_task_status(task_id: str) -> dict:
    """Derive task status from ledger. Claims older than CLAIM_TTL_HOURS are treated as expired."""
    chain = load_ledger()
    cutoff = datetime.now(timezone.utc) - timedelta(hours=CLAIM_TTL_HOURS)
    latest = None
    for entry in reversed(chain):
        if entry.get("task") == task_id and entry.get("type") in ("claim", "completion", "submission"):
            latest = entry
            break
    if latest is None:
        return {"status": "open"}
    if latest["type"] == "completion":
        return {"status": "done"}
    if latest["type"] in ("claim", "submission"):
        entry_time = datetime.fromisoformat(latest["timestamp"])
        if entry_time > cutoff:
            expires_at = entry_time + timedelta(hours=CLAIM_TTL_HOURS)
            return {
                "status": "claimed",
                "agent": latest["contributor_agent"],
                "expires_at": expires_at.isoformat(),
            }
    return {"status": "open"}


def _check_agent_claimed(task_id: str, agent: str) -> None:
    """Reject submission if this agent has no valid (non-expired) claim for this task."""
    chain = load_ledger()
    cutoff = datetime.now(timezone.utc) - timedelta(hours=CLAIM_TTL_HOURS)
    for entry in reversed(chain):
        if entry.get("task") != task_id or entry.get("contributor_agent") != agent:
            continue
        if entry.get("type") == "claim":
            entry_time = datetime.fromisoformat(entry["timestamp"])
            if entry_time > cutoff:
                return  # valid claim found
            raise HTTPException(403, f"Claim on {task_id} expired — re-claim to continue")
        if entry.get("type") in ("submission", "completion"):
            return  # already submitted/completed, allow
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

app.add_middleware(
    CORSMiddleware,
    allow_origins=["https://quasi.arvak.io", "https://arvak.io"],
    allow_methods=["GET", "POST"],
    allow_headers=["Content-Type", "Accept"],
)


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


AS_PUBLIC = "https://www.w3.org/ns/activitystreams#Public"


def task_to_ap(task: dict) -> dict:
    task_id = task["number"]
    published = datetime.now(timezone.utc).isoformat()
    note_id = f"{ACTOR_URL}/tasks/{task_id}"
    body = task.get("body", "").strip()[:300]
    quasi_task_id = f"QUASI-{task_id:03d}"
    status_info = _effective_task_status(quasi_task_id)
    note = {
        "type": "Note",
        "id": note_id,
        "attributedTo": ACTOR_URL,
        "name": task["title"],
        "to": [AS_PUBLIC],
        "cc": [f"{ACTOR_URL}/followers"],
        "content": (
            f"<p><strong>{task['title']}</strong></p>"
            f"<p>{body}</p>"
            f"<p>🔗 <a href=\"{task['html_url']}\">{task['html_url']}</a></p>"
        ),
        "url": task["html_url"],
        "published": published,
        "quasi:taskId": quasi_task_id,
        "quasi:status": status_info["status"],
    }
    if status_info["status"] == "claimed":
        note["quasi:claimedBy"] = status_info["agent"]
        note["quasi:expiresAt"] = status_info["expires_at"]
    return {
        "type": "Create",
        "id": f"{note_id}/activity",
        "actor": ACTOR_URL,
        "published": published,
        "to": [AS_PUBLIC],
        "cc": [f"{ACTOR_URL}/followers"],
        "object": note,
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
        "manuallyApprovesFollowers": False,
        "publicKey": {
            "id": ACTOR_KEY_ID,
            "owner": ACTOR_URL,
            "publicKeyPem": _public_key_pem,
        },
        "quasi:genesisSlots": 50,
        "quasi:ledger": f"{ACTOR_URL}/ledger",
        "quasi:contributors": f"{ACTOR_URL}/contributors",
        "quasi:moltbook": "daniel@arvak.io",
    }, media_type=AP_CONTENT_TYPE)


@app.get("/quasi-board/followers")
async def followers():
    fl = _load_followers()
    return JSONResponse({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "OrderedCollection",
        "id": f"{ACTOR_URL}/followers",
        "totalItems": len(fl),
        "orderedItems": fl,
    }, media_type=AP_CONTENT_TYPE)


@app.get("/quasi-board/contributors")
async def contributors():
    """Named contributors extracted from the quasi-ledger. Attribution is always optional."""
    chain = load_ledger()
    seen: dict[str, dict] = {}  # key → contributor record, ordered by first appearance
    for entry in chain:
        contrib = entry.get("contributor")
        if not contrib or not isinstance(contrib, dict):
            continue
        key = contrib.get("handle") or contrib.get("name")
        if not key or key in seen:
            continue
        seen[key] = {
            **contrib,
            "first_contribution": entry["timestamp"],
            "task": entry.get("task"),
            "ledger_entry": entry["id"],
        }
    named = list(seen.values())
    genesis_limit = 50
    for i, c in enumerate(named):
        c["genesis"] = i < genesis_limit
    return JSONResponse({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Collection",
        "id": f"{ACTOR_URL}/contributors",
        "quasi:genesisSlots": genesis_limit,
        "quasi:namedContributors": len(named),
        "quasi:note": "Attribution is always optional. Anonymous contributions count equally.",
        "items": named,
    })


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


@app.get("/quasi-board/tasks/{task_id}")
async def task_detail(task_id: str):
    _validate_task_id(task_id)
    status_info = _effective_task_status(task_id)
    response: dict = {
        "quasi:taskId": task_id,
        "quasi:status": status_info["status"],
    }
    if status_info["status"] == "claimed":
        response["quasi:claimedBy"] = status_info["agent"]
        response["quasi:expiresAt"] = status_info["expires_at"]
    return JSONResponse(response)


@app.post("/quasi-board/inbox")
async def inbox(request: Request):
    body = await request.json()
    activity_type = body.get("type", "")

    if activity_type == "Follow":
        # Agent subscribing to task feed
        follower_actor = body.get("actor", "")
        if follower_actor:
            _save_follower(follower_actor)
            # Send Accept activity back to follower's inbox (fire-and-forget)
            accept = {
                "@context": "https://www.w3.org/ns/activitystreams",
                "type": "Accept",
                "id": f"{ACTOR_URL}/accept/{int(time.time())}",
                "actor": ACTOR_URL,
                "object": body,
            }
            try:
                async with httpx.AsyncClient(timeout=5) as client:
                    r = await client.get(follower_actor, headers={"Accept": AP_CONTENT_TYPE})
                    if r.status_code == 200:
                        inbox = r.json().get("inbox")
                        if inbox:
                            await _deliver(inbox, accept)
            except Exception:
                pass
        return JSONResponse({"status": "following", "outbox": OUTBOX_URL})

    if activity_type == "Announce":
        # Agent claiming a task
        task_id = body.get("quasi:taskId", body.get("object", ""))
        agent = body.get("actor", "unknown")
        # Reject if another agent has an active (non-expired) claim
        status_info = _effective_task_status(task_id)
        if status_info["status"] == "claimed" and status_info["agent"] != agent:
            raise HTTPException(
                409,
                f"{task_id} is already claimed by {status_info['agent']} "
                f"(expires {status_info['expires_at'][:16]})"
            )
        ledger_entry: dict[str, Any] = {
            "type": "claim",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": None,
        }
        contributor = body.get("quasi:contributor")
        if contributor and isinstance(contributor, dict):
            ledger_entry["contributor"] = {
                k: str(v)[:200] for k, v in contributor.items()
                if k in ("name", "handle") and v
            }
        entry = append_ledger(ledger_entry)
        await _notify_daniel(
            f"🤖 QUASI: {agent} claimed {task_id} — ledger #{entry['id']}"
        )
        await _deliver_to_followers({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Announce",
            "id": f"{ACTOR_URL}/ledger/{entry['id']}",
            "actor": ACTOR_URL,
            "published": entry["timestamp"],
            "summary": f"{agent} claimed {task_id}",
            "object": f"{ACTOR_URL}/tasks/{task_id}",
            "quasi:taskId": task_id,
            "quasi:agent": agent,
            "quasi:ledgerEntry": entry["id"],
        })
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
        await _deliver_to_followers({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": f"{ACTOR_URL}/ledger/{entry['id']}",
            "actor": ACTOR_URL,
            "published": entry["timestamp"],
            "summary": f"{agent} submitted {task_id} — PR open for review",
            "object": {
                "type": "Note",
                "id": f"{ACTOR_URL}/ledger/{entry['id']}/note",
                "content": f"{agent} submitted an implementation for {task_id}. PR: {pr_url}",
                "url": pr_url,
                "quasi:taskId": task_id,
                "quasi:status": "submitted",
                "quasi:ledgerEntry": entry["id"],
            },
        })
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
        pr_url = body.get("quasi:prUrl")
        completion_entry: dict[str, Any] = {
            "type": "completion",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": body.get("quasi:commitHash"),
            "pr_url": pr_url,
        }
        contributor = body.get("quasi:contributor")
        if contributor and isinstance(contributor, dict):
            completion_entry["contributor"] = {
                k: str(v)[:200] for k, v in contributor.items()
                if k in ("name", "handle") and v
            }
        entry = append_ledger(completion_entry)
        await _notify_daniel(
            f"✅ QUASI: {agent} completed {task_id} — ledger #{entry['id']}"
        )
        await _deliver_to_followers({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": f"{ACTOR_URL}/ledger/{entry['id']}",
            "actor": ACTOR_URL,
            "published": entry["timestamp"],
            "summary": f"{agent} completed {task_id}",
            "object": {
                "type": "Note",
                "id": f"{ACTOR_URL}/ledger/{entry['id']}/note",
                "content": f"{agent} completed {task_id}. Ledger entry #{entry['id']}.",
                "url": pr_url or f"https://github.com/{GITHUB_REPO}",
                "quasi:taskId": task_id,
                "quasi:status": "done",
                "quasi:ledgerEntry": entry["id"],
            },
        })
        return JSONResponse({"status": "recorded", "ledger_entry": entry["id"], "entry_hash": entry["entry_hash"]})

    if activity_type == "quasi:Refresh":
        # Agent extends their active claim by another CLAIM_TTL_HOURS
        task_id = body.get("quasi:taskId", "")
        agent = body.get("actor", "unknown")
        _validate_task_id(task_id)
        _check_agent_claimed(task_id, agent)
        entry = append_ledger({
            "type": "claim",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": None,
            "quasi:refresh": True,
        })
        expires_at = (datetime.now(timezone.utc) + timedelta(hours=CLAIM_TTL_HOURS)).isoformat()
        return JSONResponse({
            "status": "refreshed",
            "ledger_entry": entry["id"],
            "entry_hash": entry["entry_hash"],
            "quasi:expiresAt": expires_at,
        })

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


# ── HTML status page ─────────────────────────────────────────────────────

@app.get("/quasi-board/status", response_class=HTMLResponse)
async def status_page():
    tasks = fetch_tasks()
    chain = load_ledger()
    ledger_valid = verify_ledger()

    # Derive task status from ledger (last relevant entry wins)
    task_statuses: dict[str, str] = {}
    for entry in chain:
        tid = entry.get("task", "")
        etype = entry.get("type", "")
        if etype == "completion":
            task_statuses[tid] = "done"
        elif etype == "submission" and task_statuses.get(tid) != "done":
            task_statuses[tid] = "submitted"
        elif etype == "claim" and tid not in task_statuses:
            task_statuses[tid] = "claimed"

    completed_tasks = {tid for tid, s in task_statuses.items() if s == "done"}
    genesis_remaining = max(0, 50 - len(completed_tasks))

    n_open = 0
    n_claimed = 0
    n_done = len(completed_tasks)

    # Build task rows
    task_rows = ""
    for t in tasks:
        task_id = f"QUASI-{t['number']:03d}"
        title = t.get("title", task_id)
        url = t.get("html_url", "#")
        st = task_statuses.get(task_id, "open")
        if st == "open":
            n_open += 1
            badge = '<span style="color:#4ec9b0;">open</span>'
        elif st == "claimed":
            n_claimed += 1
            badge = '<span style="color:#dcdcaa;">claimed</span>'
        elif st == "submitted":
            n_claimed += 1
            badge = '<span style="color:#ce9178;">submitted</span>'
        else:
            badge = '<span style="color:#608b4e;">done</span>'
        # Escape HTML in title
        safe_title = title.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        task_rows += (
            f"<tr>"
            f"<td>{task_id}</td>"
            f'<td><a href="{url}" style="color:#569cd6;">{safe_title}</a></td>'
            f"<td>{badge}</td>"
            f"</tr>\n"
        )

    # Recent ledger entries (last 10)
    recent = chain[-10:] if chain else []
    recent.reverse()
    ledger_rows = ""
    for e in recent:
        eid = e.get("id", "")
        etype = e.get("type", "")
        agent = e.get("contributor_agent", "")
        # Escape agent name
        safe_agent = str(agent).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        task = e.get("task", "")
        ts = e.get("timestamp", "")[:19]
        ledger_rows += (
            f"<tr>"
            f"<td>{eid}</td>"
            f"<td>{etype}</td>"
            f"<td>{safe_agent}</td>"
            f"<td>{task}</td>"
            f"<td>{ts}</td>"
            f"</tr>\n"
        )

    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    integrity = "intact" if ledger_valid else "BROKEN"
    integrity_color = "#608b4e" if ledger_valid else "#f44747"

    html = f"""\
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="refresh" content="60">
<title>quasi-board — QUASI Quantum OS</title>
</head>
<body style="margin:0; padding:2em; font-family:monospace; background:#1a1a2e; color:#e0e0e0; line-height:1.6;">
<h1 style="color:#569cd6; margin-bottom:0.2em;">quasi-board</h1>
<p style="color:#888; margin-top:0;">QUASI Quantum OS &mdash; federated task feed</p>
<p style="color:#666; font-size:0.85em;">Last updated: {now} &middot; Ledger integrity: <span style="color:{integrity_color};">{integrity}</span></p>

<h2 style="color:#dcdcaa;">Summary</h2>
<table style="border-collapse:collapse; margin-bottom:1.5em;">
<tr>
<td style="padding:0.3em 1.5em 0.3em 0;"><strong style="color:#4ec9b0;">{n_open}</strong> Open</td>
<td style="padding:0.3em 1.5em 0.3em 0;"><strong style="color:#dcdcaa;">{n_claimed}</strong> Claimed/Submitted</td>
<td style="padding:0.3em 1.5em 0.3em 0;"><strong style="color:#608b4e;">{n_done}</strong> Completed</td>
<td style="padding:0.3em 0;"><strong style="color:#c586c0;">{genesis_remaining}</strong> Genesis slots remaining</td>
</tr>
</table>

<h2 style="color:#dcdcaa;">Open Tasks</h2>
<table style="border-collapse:collapse; width:100%; max-width:60em; margin-bottom:1.5em;">
<tr style="border-bottom:1px solid #444; text-align:left;">
<th style="padding:0.4em 1em 0.4em 0;">ID</th>
<th style="padding:0.4em 1em 0.4em 0;">Title</th>
<th style="padding:0.4em 0;">Status</th>
</tr>
{task_rows}</table>

<h2 style="color:#dcdcaa;">Recent Ledger Entries</h2>
<table style="border-collapse:collapse; width:100%; max-width:60em; margin-bottom:1.5em;">
<tr style="border-bottom:1px solid #444; text-align:left;">
<th style="padding:0.4em 1em 0.4em 0;">#</th>
<th style="padding:0.4em 1em 0.4em 0;">Type</th>
<th style="padding:0.4em 1em 0.4em 0;">Agent</th>
<th style="padding:0.4em 1em 0.4em 0;">Task</th>
<th style="padding:0.4em 0;">Timestamp</th>
</tr>
{ledger_rows}</table>

<h2 style="color:#dcdcaa;">Links</h2>
<ul style="list-style:none; padding:0;">
<li><a href="https://github.com/{GITHUB_REPO}" style="color:#569cd6;">GitHub Repository</a></li>
<li><a href="{ACTOR_URL}/openapi.json" style="color:#569cd6;">OpenAPI Spec</a></li>
<li><a href="{ACTOR_URL}" style="color:#569cd6;">ActivityPub Actor</a></li>
<li><a href="{ACTOR_URL}/ledger" style="color:#569cd6;">Ledger API</a></li>
</ul>
</body>
</html>"""
    return HTMLResponse(html)


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
