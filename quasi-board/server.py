import base64
import hashlib
import json
import os
import httpx
import fastapi
from fastapi import FastAPI, Header, HTTPException, Request
from fastapi.responses import JSONResponse
from fastapi.middleware.cors import CORSMiddleware
from pathlib import Path

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
PROPOSALS_FILE = Path("/home/vops/quasi-board/proposals.json")

AP_CONTENT_TYPE = "application/activity+json"

async def _deliver(inbox_url: str, activity: dict) -> None:
    """"POST a signed ActivityPub activity to a remote inbox. Fire-and-forget.""""
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            await client.post(inbox_url, content=json.dumps(activity), headers={"Content-Type": AP_CONTENT_TYPE, "Accept": AP_CONTENT_TYPE})
    except Exception:
        pass  # delivery is best-effort

async def _deliver_to_followers(activity: dict) -> None:
    """"Deliver an activity to all known followers' inboxes.""""
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

async def _deliver(inbox_url: str, activity: dict) -> None:
    """"POST a signed ActivityPub activity to a remote inbox. Fire-and-forget.""""
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            await client.post(inbox_url, content=json.dumps(activity), headers={"Content-Type": AP_CONTENT_TYPE, "Accept": AP_CONTENT_TYPE})
    except Exception:
        pass  # delivery is best-effort
