import hashlib
import json
from datetime import datetime, timezone, timedelta
from unittest.mock import patch, AsyncMock

import pytest

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


def _make_entry(id, type, task, agent, minutes_ago=0):
    ts = (datetime.now(timezone.utc) - timedelta(minutes=minutes_ago)).isoformat()
    entry = {
        "id": id, "type": type, "task": task,
        "contributor_agent": agent, "timestamp": ts,
        "commit_hash": None, "pr_url": None, "prev_hash": "0" * 64,
    }
    raw = json.dumps({k: v for k, v in entry.items() if k != "entry_hash"}, sort_keys=True)
    entry["entry_hash"] = hashlib.sha256(raw.encode()).hexdigest()
    return entry


@pytest.mark.anyio
async def test_refresh_extends_active_claim():
    from httpx import ASGITransport, AsyncClient
    from server import app, CLAIM_TTL_HOURS

    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=5)]
    refreshed_entry = _make_entry(2, "claim", "QUASI-001", "agent-a", minutes_ago=0)
    refreshed_entry["quasi:refresh"] = True

    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", return_value=refreshed_entry):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "quasi:Refresh",
                "actor": "agent-a",
                "quasi:taskId": "QUASI-001",
            })

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "refreshed"
    assert "quasi:expiresAt" in data


@pytest.mark.anyio
async def test_refresh_without_claim_returns_403():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "quasi:Refresh",
                "actor": "agent-a",
                "quasi:taskId": "QUASI-001",
            })

    assert resp.status_code == 403


@pytest.mark.anyio
async def test_refresh_with_expired_claim_returns_403():
    from httpx import ASGITransport, AsyncClient
    from server import app

    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=60)]

    with patch("server.load_ledger", return_value=chain):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "quasi:Refresh",
                "actor": "agent-a",
                "quasi:taskId": "QUASI-001",
            })

    assert resp.status_code == 403
