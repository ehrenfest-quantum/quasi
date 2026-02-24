from unittest.mock import patch

import pytest

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


@pytest.mark.anyio
async def test_stats_empty_ledger():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=[]), \
         patch("server.verify_ledger", return_value=True), \
         patch("server._fetch_open_issue_count", return_value=10):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/stats")

    assert resp.status_code == 200
    data = resp.json()
    assert data["tasks_open"] == 10
    assert data["tasks_claimed"] == 0
    assert data["tasks_done"] == 0
    assert data["contributors_named"] == 0
    assert data["genesis_slots_remaining"] == 50
    assert data["ledger_entries"] == 0
    assert data["ledger_valid"] is True


@pytest.mark.anyio
async def test_stats_with_ledger_entries():
    from httpx import ASGITransport, AsyncClient
    from server import app

    chain = [
        {"id": 1, "type": "claim", "task": "QUASI-001", "contributor_agent": "bot-a",
         "entry_hash": "a" * 64, "prev_hash": "0" * 64, "timestamp": "2026-01-01T00:00:00Z"},
        {"id": 2, "type": "completion", "task": "QUASI-001", "contributor_agent": "bot-a",
         "contributor": {"name": "Alice", "handle": "@alice@fosstodon.org"},
         "entry_hash": "b" * 64, "prev_hash": "a" * 64, "timestamp": "2026-01-02T00:00:00Z"},
        {"id": 3, "type": "claim", "task": "QUASI-002", "contributor_agent": "bot-b",
         "entry_hash": "c" * 64, "prev_hash": "b" * 64, "timestamp": "2026-01-03T00:00:00Z"},
    ]

    with patch("server.load_ledger", return_value=chain), \
         patch("server.verify_ledger", return_value=True), \
         patch("server._fetch_open_issue_count", return_value=20):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/stats")

    assert resp.status_code == 200
    data = resp.json()
    assert data["tasks_done"] == 1       # QUASI-001 completed
    assert data["tasks_claimed"] == 1    # QUASI-002 claimed, not yet done
    assert data["tasks_open"] == 18      # 20 - 1 done - 1 claimed
    assert data["contributors_named"] == 1   # Alice
    assert data["genesis_slots_remaining"] == 49
    assert data["ledger_entries"] == 3
    assert data["ledger_valid"] is True
