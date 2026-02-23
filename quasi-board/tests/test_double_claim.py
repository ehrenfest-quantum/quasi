import hashlib
import json
from datetime import datetime, timezone, timedelta
from unittest.mock import patch, AsyncMock

import pytest

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


def _make_entry(id, type, task, agent, hours_ago=0):
    ts = (datetime.now(timezone.utc) - timedelta(hours=hours_ago)).isoformat()
    entry = {
        "id": id, "type": type, "task": task,
        "contributor_agent": agent, "timestamp": ts,
        "commit_hash": None, "pr_url": None, "prev_hash": "0" * 64,
    }
    raw = json.dumps({k: v for k, v in entry.items() if k != "entry_hash"}, sort_keys=True)
    entry["entry_hash"] = hashlib.sha256(raw.encode()).hexdigest()
    return entry


@pytest.mark.anyio
async def test_double_claim_returns_409():
    from httpx import ASGITransport, AsyncClient
    from server import app

    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", hours_ago=1)]

    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger") as mock_append, \
         patch("server._notify_daniel", new_callable=AsyncMock), \
         patch("server._deliver_to_followers", new_callable=AsyncMock):

        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "Announce",
                "actor": "agent-b",
                "quasi:taskId": "QUASI-001",
            })

        assert resp.status_code == 409
        mock_append.assert_not_called()


@pytest.mark.anyio
async def test_claim_after_expiry_allowed():
    from httpx import ASGITransport, AsyncClient
    from server import app

    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", hours_ago=25)]
    new_entry = _make_entry(2, "claim", "QUASI-001", "agent-b", hours_ago=0)

    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", return_value=new_entry), \
         patch("server._notify_daniel", new_callable=AsyncMock), \
         patch("server._deliver_to_followers", new_callable=AsyncMock):

        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "Announce",
                "actor": "agent-b",
                "quasi:taskId": "QUASI-001",
            })

        assert resp.status_code == 200


@pytest.mark.anyio
async def test_same_agent_reclaim_allowed():
    from httpx import ASGITransport, AsyncClient
    from server import app

    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", hours_ago=1)]
    new_entry = _make_entry(2, "claim", "QUASI-001", "agent-a", hours_ago=0)

    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", return_value=new_entry), \
         patch("server._notify_daniel", new_callable=AsyncMock), \
         patch("server._deliver_to_followers", new_callable=AsyncMock):

        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json={
                "type": "Announce",
                "actor": "agent-a",
                "quasi:taskId": "QUASI-001",
            })

        assert resp.status_code == 200
