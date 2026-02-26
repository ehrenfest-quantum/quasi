#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 Daniel Hinderink
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
from email.utils import formatdate
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import httpx
from fastapi import FastAPI, Header, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse, PlainTextResponse
import hmac as _hmac
import re as _re

DOMAIN = os.environ.get("QUASI_DOMAIN", "gawain.valiant-quantum.com")
ACTOR_URL = f"https://{DOMAIN}/quasi-board"
OUTBOX_URL = f"{ACTOR_URL}/outbox"
INBOX_URL = f"{ACTOR_URL}/inbox"
_DATA_DIR = Path(os.environ.get("QUASI_DATA_DIR", "/home/vops/quasi-board"))
_LEDGER_DIR = Path(os.environ.get("QUASI_LEDGER_DIR", "/home/vops/quasi-ledger"))
LEDGER_FILE = _LEDGER_DIR / "ledger.json"
OPENAPI_SPEC = Path(__file__).parent / "spec" / "openapi.json"
GITHUB_REPO = os.environ.get("QUASI_GITHUB_REPO", "ehrenfest-quantum/quasi")
GITHUB_TOKEN_FILE = _DATA_DIR / ".github_token"
MATRIX_CREDS_FILE = _DATA_DIR / "matrix_credentials.json"
MATRIX_ROOM_ID = "!CerauaaS111HsAzJXI:gawain.valiant-quantum.com"
ACTOR_KEY_FILE = _DATA_DIR / "keys" / "actor.pem"
FOLLOWERS_FILE = _DATA_DIR / "followers.json"
PROPOSALS_FILE = Path("/home/vops/quasi-board/proposals.json")
AGENT_TOKENS_FILE = Path("/home/vops/quasi-board/agent-tokens.json")
PENDING_MERGES_FILE = Path("/home/vops/quasi-board/pending-merges.json")
ACTOR_KEY_ID = f"{ACTOR_URL}#main-key"

AP_CONTENT_TYPE = "application/activity+json"


app = FastAPI(title="quasi-board", version="0.1.0")
app.add_middleware(CORSMiddleware, allow_origins=["*"], allow_methods=["GET"], allow_headers=["*"])


# Prometheus-compatible metrics for quasi-board
@app.get("/quasi-board/metrics", response_class=PlainTextResponse)
def metrics() -> PlainTextResponse:
    """Return Prometheus-compatible metrics for quasi-board.

    Returns:
        PlainTextResponse: Prometheus-formatted metrics including:
            - Task counts by status (open/claimed/done)
            - Total ledger entries
            - Remaining genesis slots
            - Active claims count
    """
    """Return Prometheus-compatible metrics."""
    tasks = json.loads((Path(__file__).parent / 'testdata/outbox.json').read_text()).get('orderedItems', [])
    task_status_counts = {'open': 0, 'claimed': 0, 'done': 0}
    for item in tasks:
        t = item.get('object', item) if item.get('type') == 'Create' else item
        status = t.get('quasi:status', 'open')
        if status in task_status_counts:
            task_status_counts[status] += 1
    ledger = json.loads((Path(__file__).parent / 'testdata/ledger.json').read_text())
    ledger_entries_total = len(ledger.get('entries', []))
    genesis_slots_remaining = 50 - len(ledger.get('contributors', []))
    claims_active = sum(1 for task in tasks if task.get('quasi:status') == 'claimed')
    metrics_text = """
# HELP quasi_tasks_total Count of tasks by status
# TYPE quasi_tasks_total gauge
"""
    for status, count in task_status_counts.items():
        metrics_text += f'quasi_tasks_total{{status="{status}"}} {count}\n'
    metrics_text += f"""
# HELP quasi_ledger_entries_total Total number of ledger entries
# TYPE quasi_ledger_entries_total gauge
quasi_ledger_entries_total {ledger_entries_total}\n"""
    metrics_text += f"""
# HELP quasi_genesis_slots_remaining Remaining genesis slots (50 - named contributors)
# TYPE quasi_genesis_slots_remaining gauge
quasi_genesis_slots_remaining {genesis_slots_remaining}\n"""
    metrics_text += f"""
# HELP quasi_claims_active Number of currently active (claimed) tasks
# TYPE quasi_claims_active gauge
quasi_claims_active {claims_active}\n"""
    return metrics_text


# ── HTTP Signatures ───────────────────────────────────────────────────────────

def _load_or_create_keys() -> tuple[Any, str]:
    """Load or generate RSA-2048 key pair for HTTP signatures.

    Returns:
        tuple: (private_key, public_key_pem) where:
            private_key: cryptography.hazmat.primitives.asymmetric.rsa.RSAPrivateKey
            public_key_pem: PEM-encoded public key as str
    """
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


# ── Proposals ─────────────────────────────────────────────────────────────────

def _load_proposals() -> list[dict]:
    if not PROPOSALS_FILE.exists():
        return []
    return json.loads(PROPOSALS_FILE.read_text()).get("proposals", [])


def _save_proposals(proposals: list[dict]) -> None:
    PROPOSALS_FILE.parent.mkdir(parents=True, exist_ok=True)
    PROPOSALS_FILE.write_text(json.dumps({"proposals": proposals}, indent=2))


def _admin_token() -> str:
    return os.environ.get("QUASI_ADMIN_TOKEN", "")


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


# Paths that require human review before auto-merge (softer gate than _BLOCKED)
_REVIEW_REQUIRED_PREFIXES = (
    "quasi-board/",   # board server — sensitive but agents may legitimately patch
    ".github/",       # CI/CD workflows
    "spec/",          # core specification
)

_REVIEW_REQUIRED_EXACT = {
    "README.md",
}


def _requires_human_review(changed_files: dict) -> bool:
    """Return True if any changed file path requires a human review gate."""
    for path in changed_files:
        clean = "/".join(
            p for p in path.replace("\\", "/").split("/")
            if p not in ("", ".", "..")
        )
        if clean in _REVIEW_REQUIRED_EXACT:
            return True
        for prefix in _REVIEW_REQUIRED_PREFIXES:
            if clean.startswith(prefix) or clean == prefix.rstrip("/"):
                return True
    return False


def _load_pending_merges() -> list:
    """Load the list of PRs pending human review from disk."""
    if PENDING_MERGES_FILE.exists():
        try:
            return json.loads(PENDING_MERGES_FILE.read_text())
        except Exception:
            pass
    return []


def _save_pending_merges(merges: list) -> None:
    """Persist the pending merges list to disk."""
    PENDING_MERGES_FILE.parent.mkdir(parents=True, exist_ok=True)
    PENDING_MERGES_FILE.write_text(json.dumps(merges, indent=2))


async def _fetch_github_issue(issue_number: int) -> dict | None:
    """Fetch a single GitHub issue by number. Returns None on failure."""
    token = _github_token()
    headers: dict[str, str] = {"Accept": "application/vnd.github+json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            resp = await client.get(
                f"https://api.github.com/repos/{GITHUB_REPO}/issues/{issue_number}",
                headers=headers,
            )
            if resp.status_code == 200:
                return resp.json()
    except Exception:
        pass
    return None


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


CLAIM_TTL_MINUTES = 30


def _effective_task_status(task_id: str) -> dict:
    """Return the effective status of a task based on the ledger.

    Returns one of:
      {"status": "open"}
      {"status": "claimed", "agent": str, "expires_at": str}
      {"status": "done", "agent": str}
    """
    from datetime import datetime, timezone, timedelta
    chain = load_ledger()
    relevant = [e for e in chain if e.get("task") == task_id]

    last_claim = None
    last_claim_ts = None
    last_activity_ts = None

    for entry in relevant:
        t = entry.get("type")
        ts_str = entry.get("timestamp")
        try:
            ts = datetime.fromisoformat(ts_str)
        except Exception:
            continue

        if t == "completion":
            return {"status": "done", "agent": entry.get("contributor_agent")}

        if t == "claim":
            last_claim = entry
            last_claim_ts = ts
            last_activity_ts = ts

        if t == "submission" and last_claim_ts is not None:
            # Submission refreshes the TTL window
            last_activity_ts = ts

    if last_claim is None:
        return {"status": "open"}

    now = datetime.now(timezone.utc)
    if last_activity_ts.tzinfo is None:
        last_activity_ts = last_activity_ts.replace(tzinfo=timezone.utc)

    expires_at = last_activity_ts + timedelta(minutes=CLAIM_TTL_MINUTES)
    if now > expires_at:
        return {"status": "open"}

    return {
        "status": "claimed",
        "agent": last_claim.get("contributor_agent"),
        "expires_at": expires_at.isoformat(),
    }


# ── Agent token store (C2S auth) ──────────────────────────────────────────────


def _load_agent_tokens() -> dict:
    """Return {token: agent_id} mapping from disk."""
    if AGENT_TOKENS_FILE.exists():
        return json.loads(AGENT_TOKENS_FILE.read_text())
    return {}


def _save_agent_tokens(tokens: dict) -> None:
    AGENT_TOKENS_FILE.parent.mkdir(parents=True, exist_ok=True)
    AGENT_TOKENS_FILE.write_text(json.dumps(tokens, indent=2))


def _resolve_c2s_agent(authorization: str) -> str:
    """Resolve a Bearer token to an agent_id. Raises HTTP 401 if invalid."""
    if not authorization or not authorization.startswith("Bearer "):
        raise HTTPException(401, "C2S requires Authorization: Bearer <token>")
    token = authorization[7:].strip()
    tokens = _load_agent_tokens()
    if token not in tokens:
        raise HTTPException(401, "Unknown or revoked agent token")
    return tokens[token]


def _check_agent_claimed(task_id: str, agent: str) -> None:
    """Reject submission if this agent has no active (non-expired) claim for this task."""
    effective = _effective_task_status(task_id)
    if effective["status"] == "claimed" and effective.get("agent") == agent:
        return
    if effective["status"] == "open":
        # Check if there was a claim that expired
        chain = load_ledger()
        had_claim = any(
            e.get("type") == "claim"
            and e.get("task") == task_id
            and e.get("contributor_agent") == agent
            for e in chain
        )
        if had_claim:
            raise HTTPException(403, f"Claim for {task_id} by {agent!r} has expired — re-claim first")
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
        {
            "number": 1, "title": "QUASI-001: Ehrenfest CBOR Schema",
            "html_url": f"https://github.com/{GITHUB_REPO}/issues/1",
            "body": "Define CBOR/CDDL schema for Ehrenfest base types.",
        },
        {
            "number": 2, "title": "QUASI-002: HAL Contract Python Bindings",
            "html_url": f"https://github.com/{GITHUB_REPO}/issues/2",
            "body": "Python FFI for the HAL Contract.",
        },
        {
            "number": 3, "title": "QUASI-003: quasi-board ActivityPub Prototype",
            "html_url": f"https://github.com/{GITHUB_REPO}/issues/3",
            "body": "Federated task feed using ActivityPub.",
        },
    ]


AS_PUBLIC = "https://www.w3.org/ns/activitystreams#Public"


def task_to_ap(task: dict) -> dict:
    task_id = task["number"]
    published = datetime.now(timezone.utc).isoformat()
    note_id = f"{ACTOR_URL}/tasks/{task_id}"
    body = task.get("body", "").strip()[:300]
    quasi_task_id = f"QUASI-{task_id:03d}"
    effective = _effective_task_status(quasi_task_id)
    note: dict[str, Any] = {
        "type": "Note",
        "id": note_id,
        "attributedTo": ACTOR_URL,
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
        "quasi:status": effective["status"],
    }
    if effective["status"] == "claimed":
        note["quasi:claimedBy"] = effective.get("agent")
        note["quasi:expiresAt"] = effective.get("expires_at")
    elif effective["status"] == "done":
        note["quasi:claimedBy"] = effective.get("agent")
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
        "summary": (
            "QUASI Quantum OS — federated task feed. Build the first Quantum OS. "
            "Ehrenfest language. Afana compiler. Urns packages. "
            "https://github.com/ehrenfest-quantum/quasi"
        ),
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


async def _process_activity(body: dict) -> JSONResponse:
    """Core ActivityPub activity processor — shared by C2S outbox and S2S inbox."""
    activity_type = body.get("type", "")
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

    if activity_type == "quasi:Refresh":
        # Agent refreshing an active claim TTL
        task_id = body.get("quasi:taskId", "")
        agent = body.get("actor", "unknown")
        effective = _effective_task_status(task_id)
        if effective["status"] != "claimed" or effective.get("agent") != agent:
            raise HTTPException(403, f"Agent {agent!r} has no active claim on {task_id}")
        refresh_entry = append_ledger({
            "type": "claim",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": None,
            "quasi:refresh": True,
        })
        new_effective = _effective_task_status(task_id)
        return JSONResponse({
            "status": "refreshed",
            "quasi:expiresAt": new_effective.get("expires_at", ""),
            "ledger_entry": refresh_entry["id"],
            "entry_hash": refresh_entry["entry_hash"],
        })

    if activity_type == "Announce":
        # Agent claiming a task
        task_id = body.get("quasi:taskId", body.get("object", ""))
        agent = body.get("actor", "unknown")
        # Reject if task is already actively claimed by a different agent
        effective = _effective_task_status(task_id)
        if effective["status"] == "claimed" and effective.get("agent") != agent:
            raise HTTPException(409, f"{task_id} is already claimed by {effective['agent']!r}")
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
            "published": entry.get("timestamp", ""),
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
        review_required = _requires_human_review(files)

        entry = append_ledger({
            "type": "submission",
            "contributor_agent": agent,
            "task": task_id,
            "commit_hash": None,
            "pr_url": pr_url,
        })

        if review_required:
            # Queue for human review — do not auto-merge
            pending = _load_pending_merges()
            pr_number_match = __import__("re").search(r"/pull/(\d+)", pr_url)
            pr_num = int(pr_number_match.group(1)) if pr_number_match else 0
            pending.append({
                "pr_number": pr_num,
                "pr_url": pr_url,
                "task_id": task_id,
                "agent": agent,
                "ledger_submission_id": entry["id"],
                "submitted_at": entry.get("timestamp", ""),
            })
            _save_pending_merges(pending)
            await _notify_daniel(
                f"🔍 QUASI: {agent} submitted {task_id} — REVIEW REQUIRED — PR: {pr_url} — ledger #{entry['id']}"
            )
            return JSONResponse({
                "status": "pending_human_review",
                "review_required": True,
                "pr_url": pr_url,
                "ledger_entry": entry["id"],
                "entry_hash": entry["entry_hash"],
            })

        await _notify_daniel(
            f"🤖 QUASI: {agent} submitted {task_id} — PR opened: {pr_url} — ledger #{entry['id']}"
        )
        await _deliver_to_followers({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": f"{ACTOR_URL}/ledger/{entry['id']}",
            "actor": ACTOR_URL,
            "published": entry.get("timestamp", ""),
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
            "review_required": False,
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
            "published": entry.get("timestamp", ""),
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

    if activity_type == "Create" and body.get("quasi:type") == "issue_generated":
        gen_entry: dict[str, Any] = {
            "type": "issue_generated",
            "generator_model": str(body.get("quasi:generator_model", "unknown"))[:200],
            "generator_provider": str(body.get("quasi:generator_provider", "unknown"))[:100],
            "level": body.get("quasi:level", 0),
            "issue_url": str(body.get("quasi:issueUrl", ""))[:500],
        }
        entry = append_ledger(gen_entry)
        return JSONResponse({"status": "recorded", "ledger_entry": entry["id"], "entry_hash": entry["entry_hash"]})

    if activity_type == "quasi:Propose":
        proposal_obj = body.get("object", {})
        title = str(proposal_obj.get("quasi:title", "")).strip()[:200]
        description = str(proposal_obj.get("quasi:description", "")).strip()[:2000]
        if not title or not description:
            raise HTTPException(400, "quasi:title and quasi:description are required")

        proposals = _load_proposals()
        prop_id = f"prop-{len(proposals) + 1:03d}"
        proposal: dict[str, Any] = {
            "id": prop_id,
            "title": title,
            "description": description,
            "estimated_effort": str(proposal_obj.get("quasi:estimatedEffort", ""))[:200],
            "rationale": str(proposal_obj.get("quasi:rationale", ""))[:500],
            "proposed_by": str(body.get("actor", "unknown"))[:200],
            "proposed_at": datetime.now(timezone.utc).isoformat(),
            "status": "pending",
        }
        proposals.append(proposal)
        _save_proposals(proposals)
        await _notify_daniel(f"📋 QUASI proposal: {title} from {proposal['proposed_by']}")
        return JSONResponse({"status": "proposed", "id": prop_id}, status_code=202)

    return JSONResponse({"status": "accepted"})


# ── Task status endpoint ──────────────────────────────────────────────────────


@app.post("/quasi-board/inbox")
async def inbox(request: Request):
    """S2S ActivityPub inbox — accepts federated activities from remote servers."""
    body = await request.json()
    return await _process_activity(body)


@app.get("/quasi-board/tasks/{task_id}")
async def task_status(task_id: str):
    import re as _re_local
    # Accept plain numbers (e.g. "54") and normalise to QUASI-054
    if _re_local.fullmatch(r"\d{1,6}", task_id):
        task_id = f"QUASI-{int(task_id):03d}"
    _validate_task_id(task_id)
    # Extract numeric issue number for GitHub lookup
    m = _re_local.search(r"QUASI-(\d+)", task_id)
    issue_number = int(m.group(1)) if m else None

    # Load ledger entries for this task (single load — used for both status and entries)
    chain = load_ledger()
    task_entries = [e for e in chain if e.get("task") == task_id]

    # Derive status from ledger entries (last entry type wins; no TTL for display)
    status = "open"
    claimed_by: str | None = None
    for e in task_entries:
        t = e.get("type")
        if t == "claim":
            status = "claimed"
            claimed_by = e.get("contributor_agent")
        elif t in ("completion", "merge"):
            status = "done"
            claimed_by = e.get("contributor_agent")

    result: dict[str, Any] = {
        "quasi:taskId": task_id,
        "quasi:status": status,
        "quasi:ledgerEntries": task_entries,
    }
    if status == "claimed":
        result["quasi:claimedBy"] = claimed_by
    elif status == "done":
        result["quasi:claimedBy"] = claimed_by

    # Fetch GitHub issue data (graceful degradation on failure)
    if issue_number is not None:
        github_issue = await _fetch_github_issue(issue_number)
        if github_issue is not None:
            result["task"] = github_issue

    return JSONResponse(result)


# ── Admin: pending human-review merges ───────────────────────────────────────


@app.get("/quasi-board/admin/merges")
async def list_pending_merges(request: Request):
    """Admin: list PRs waiting for human review before merge."""
    _check_admin(request)
    pending = _load_pending_merges()
    return JSONResponse({"pending": pending, "count": len(pending)})


@app.post("/quasi-board/admin/merges/{pr_number}/approve")
async def approve_merge(pr_number: int, request: Request):
    """Admin: approve a pending PR — merges it via GitHub API and records completion."""
    _check_admin(request)
    pending = _load_pending_merges()
    record = next((p for p in pending if p["pr_number"] == pr_number), None)
    if record is None:
        raise HTTPException(404, f"No pending merge for PR #{pr_number}")

    # Merge via GitHub API
    gh_token = _github_token()
    headers: dict[str, str] = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {gh_token}",
    }
    merge_url = f"https://api.github.com/repos/{GITHUB_REPO}/pulls/{pr_number}/merge"
    async with httpx.AsyncClient(timeout=30) as client:
        resp = await client.put(merge_url, headers=headers, json={"merge_method": "squash"})
        resp.raise_for_status()
        merge_data = resp.json()

    # Record completion in ledger
    completion_entry = append_ledger({
        "type": "completion",
        "contributor_agent": record["agent"],
        "task": record["task_id"],
        "commit_hash": merge_data.get("sha"),
        "pr_url": record["pr_url"],
        "review_approved_by": "admin",
    })

    # Remove from pending queue
    updated_pending = [p for p in pending if p["pr_number"] != pr_number]
    _save_pending_merges(updated_pending)

    await _notify_daniel(
        f"✅ QUASI: PR #{pr_number} ({record['task_id']}) approved and merged — ledger #{completion_entry['id']}"
    )
    return JSONResponse({
        "status": "merged",
        "task_id": record["task_id"],
        "pr_url": record["pr_url"],
        "merge_sha": merge_data.get("sha"),
        "ledger_completion": completion_entry["id"],
    })


@app.post("/quasi-board/admin/merges/{pr_number}/reject")
async def reject_merge(pr_number: int, request: Request):
    """Admin: reject a pending PR — closes it on GitHub and removes from queue."""
    _check_admin(request)
    pending = _load_pending_merges()
    record = next((p for p in pending if p["pr_number"] == pr_number), None)
    if record is None:
        raise HTTPException(404, f"No pending merge for PR #{pr_number}")

    # Close PR via GitHub API
    gh_token = _github_token()
    headers: dict[str, str] = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {gh_token}",
    }
    close_url = f"https://api.github.com/repos/{GITHUB_REPO}/pulls/{pr_number}"
    async with httpx.AsyncClient(timeout=30) as client:
        await client.patch(close_url, headers=headers, json={"state": "closed"})

    # Remove from pending queue
    updated_pending = [p for p in pending if p["pr_number"] != pr_number]
    _save_pending_merges(updated_pending)

    await _notify_daniel(
        f"❌ QUASI: PR #{pr_number} ({record['task_id']}) rejected and closed"
    )
    return JSONResponse({
        "status": "rejected",
        "task_id": record["task_id"],
        "pr_url": record["pr_url"],
    })


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


# ── Stats ──────────────────────────────────────────────────────────────────────

def _fetch_open_issue_count() -> int:
    """Return the number of open GitHub issues. Falls back to 0 on error."""
    try:
        resp = httpx.get(
            f"https://api.github.com/repos/{GITHUB_REPO}/issues",
            params={"state": "open", "per_page": 1},
            headers={"Accept": "application/vnd.github+json"},
            timeout=10,
        )
        if resp.status_code == 200:
            link = resp.headers.get("link", "")
            # GitHub paginates; last page number is in the Link header
            import re as _re
            m = _re.search(r'page=(\d+)>; rel="last"', link)
            if m:
                return int(m.group(1))
            return len(resp.json())
    except Exception:
        pass
    return 0


@app.get("/quasi-board/stats")
async def stats():
    chain = load_ledger()
    valid = verify_ledger()

    done_tasks = {e["task"] for e in chain if e.get("type") == "completion" and e.get("task")}
    claimed_tasks = {e["task"] for e in chain if e.get("type") == "claim" and e.get("task")} - done_tasks

    seen_contributors: set[str] = set()
    for entry in chain:
        contrib = entry.get("contributor")
        if contrib and isinstance(contrib, dict):
            key = contrib.get("handle") or contrib.get("name")
            if key:
                seen_contributors.add(key)

    genesis_limit = 50
    total_open = _fetch_open_issue_count()
    tasks_open = max(0, total_open - len(done_tasks) - len(claimed_tasks))

    return JSONResponse({
        "tasks_open": tasks_open,
        "tasks_claimed": len(claimed_tasks),
        "tasks_done": len(done_tasks),
        "contributors_named": len(seen_contributors),
        "genesis_slots_remaining": max(0, genesis_limit - len(done_tasks)),
        "ledger_entries": len(chain),
        "ledger_valid": valid,
    })


# ── Proposals ─────────────────────────────────────────────────────────────────

@app.get("/quasi-board/proposals")
async def proposals_list():
    """Public list of all task proposals submitted by agents."""
    props = _load_proposals()
    return JSONResponse({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Collection",
        "id": f"{ACTOR_URL}/proposals",
        "totalItems": len(props),
        "items": props,
    })


def _check_admin(request: Request) -> None:
    """Raise 401 if the request does not carry the admin bearer token."""
    token = _admin_token()
    if not token:
        raise HTTPException(401, "Admin token not configured on this server")
    auth = request.headers.get("Authorization", "")
    if auth != f"Bearer {token}":
        raise HTTPException(401, "Invalid or missing admin token")


@app.post("/quasi-board/admin/proposals/{prop_id}/accept")
async def accept_proposal(prop_id: str, request: Request):
    _check_admin(request)
    proposals = _load_proposals()
    for p in proposals:
        if p["id"] == prop_id:
            if p["status"] != "pending":
                raise HTTPException(409, f"Proposal is already '{p['status']}'")
            p["status"] = "accepted"
            p["accepted_at"] = datetime.now(timezone.utc).isoformat()
            _save_proposals(proposals)
            entry = append_ledger({
                "type": "proposal_accepted",
                "proposal_id": prop_id,
                "title": p["title"],
                "proposed_by": p["proposed_by"],
            })
            await _notify_daniel(f"✅ Proposal accepted: {p['title']} ({prop_id})")
            return JSONResponse({
                "status": "accepted",
                "proposal": p,
                "ledger_entry": entry["id"],
                "entry_hash": entry["entry_hash"],
            })
    raise HTTPException(404, f"Proposal '{prop_id}' not found")


@app.post("/quasi-board/admin/proposals/{prop_id}/reject")
async def reject_proposal(prop_id: str, request: Request):
    _check_admin(request)
    proposals = _load_proposals()
    for p in proposals:
        if p["id"] == prop_id:
            if p["status"] != "pending":
                raise HTTPException(409, f"Proposal is already '{p['status']}'")
            p["status"] = "rejected"
            p["rejected_at"] = datetime.now(timezone.utc).isoformat()
            _save_proposals(proposals)
            return JSONResponse({"status": "rejected", "proposal": p})
    raise HTTPException(404, f"Proposal '{prop_id}' not found")


# ── ActivityPub C2S outbox ────────────────────────────────────────────────────

@app.post("/quasi-board/outbox")
async def c2s_outbox(request: Request, authorization: str = Header(default="")):
    """
    ActivityPub Client-to-Server (C2S) outbox.

    Authenticated agents POST Activities here instead of to the inbox.
    The actor field is set server-side from the Bearer token — clients
    cannot spoof each other's identity.

    Supported activity types (same as inbox):
      Announce               — claim a task
      Create quasi:patch     — submit implementation (board opens PR)
      Create quasi:completion — report a completed task
      quasi:Refresh          — refresh active claim TTL
      quasi:Propose          — propose a new task

    Returns 201 Created with Location header on success.
    """
    agent = _resolve_c2s_agent(authorization)
    body = await request.json()
    # Override actor — trust the token, not the payload
    body["actor"] = agent
    response = await _process_activity(body)
    # Wrap in 201 + Location so C2S clients know the canonical entry URL
    data = response.body
    import json as _json
    parsed = _json.loads(data)
    ledger_id = parsed.get("ledger_entry")
    headers = {}
    if ledger_id:
        headers["Location"] = f"{ACTOR_URL}/ledger/{ledger_id}"
    return JSONResponse(parsed, status_code=201, headers=headers)


# ── Per-agent AP Actor profiles ───────────────────────────────────────────────

@app.get("/quasi-board/actors/{agent_id}")
async def agent_actor(agent_id: str):
    """
    ActivityPub Actor profile for a registered agent.
    Enables federated servers to discover and follow individual agents.
    """
    tokens = _load_agent_tokens()
    known_agents = set(tokens.values())
    if agent_id not in known_agents:
        raise HTTPException(404, f"Agent {agent_id!r} not registered")

    actor_url = f"https://{DOMAIN}/quasi-board/actors/{agent_id}"
    return JSONResponse(
        {
            "@context": [
                "https://www.w3.org/ns/activitystreams",
                "https://w3id.org/security/v1",
            ],
            "type": "Service",
            "id": actor_url,
            "name": agent_id,
            "preferredUsername": agent_id.replace("/", "-"),
            "url": actor_url,
            "inbox": f"{actor_url}/inbox",
            "outbox": f"{ACTOR_URL}/outbox",
            "following": f"{ACTOR_URL}/following",
            "followers": f"{ACTOR_URL}/followers",
            "quasi:ledger": f"{ACTOR_URL}/ledger",
            "quasi:agentOf": ACTOR_URL,
        },
        headers={"Content-Type": AP_CONTENT_TYPE},
    )


# ── Admin: agent token management ────────────────────────────────────────────

@app.post("/quasi-board/admin/agents")
async def register_agent(request: Request):
    """
    Admin: register an agent and issue a C2S Bearer token.

    POST body: {"agent_id": "deepseek-v3"}
    Returns:   {"agent_id": "...", "token": "..."}  (201 Created)

    The token is shown once — store it securely.
    """
    _check_admin(request)
    import secrets as _secrets
    data = await request.json()
    agent_id = str(data.get("agent_id", "")).strip()
    if not agent_id:
        raise HTTPException(400, "agent_id is required")
    tokens = _load_agent_tokens()
    for existing_agent in tokens.values():
        if existing_agent == agent_id:
            raise HTTPException(409, f"Agent {agent_id!r} already has a token — revoke first")
    token = _secrets.token_urlsafe(32)
    tokens[token] = agent_id
    _save_agent_tokens(tokens)
    return JSONResponse(
        {"agent_id": agent_id, "token": token, "c2s_outbox": f"https://{DOMAIN}/quasi-board/outbox"},
        status_code=201,
    )


@app.get("/quasi-board/admin/agents")
async def list_agents(request: Request):
    """Admin: list all registered agents (token values hidden)."""
    _check_admin(request)
    tokens = _load_agent_tokens()
    agents = sorted(set(tokens.values()))
    return JSONResponse({"agents": agents, "count": len(agents)})


@app.delete("/quasi-board/admin/agents/{agent_id}")
async def revoke_agent(agent_id: str, request: Request):
    """Admin: revoke all tokens for an agent."""
    _check_admin(request)
    tokens = _load_agent_tokens()
    to_remove = [t for t, a in tokens.items() if a == agent_id]
    if not to_remove:
        raise HTTPException(404, f"Agent {agent_id!r} not found")
    for t in to_remove:
        del tokens[t]
    _save_agent_tokens(tokens)
    return JSONResponse({"revoked": agent_id, "tokens_removed": len(to_remove)})


# ── GitHub webhook ────────────────────────────────────────────────────────────


WEBHOOK_SECRET_FILE = _DATA_DIR / ".webhook_secret"


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

    pr_body = pr.get("body") or ""
    pr_title = pr.get("title", "")
    pr_url = pr.get("html_url", "")
    pr_author = pr.get("user", {}).get("login", "unknown")
    commit_sha = pr.get("merge_commit_sha", "")

    meta = _parse_meta(pr_body)
    agent = meta.get("Contribution-Agent", pr_author)
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
