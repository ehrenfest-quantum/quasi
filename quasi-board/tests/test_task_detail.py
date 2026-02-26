from unittest.mock import patch

import pytest

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


MOCK_ISSUE = {
    "number": 54,
    "title": "QUASI-054: /tasks/{id} endpoint",
    "body": "Add a GET endpoint that returns a single task by its QUASI number.",
    "html_url": "https://github.com/ehrenfest-quantum/quasi/issues/54",
    "state": "open",
    "labels": [{"name": "L1"}],
    "created_at": "2026-02-20T00:00:00Z",
    "updated_at": "2026-02-24T00:00:00Z",
}

MOCK_LEDGER = [
    {
        "id": 1, "type": "claim", "task": "QUASI-054",
        "contributor_agent": "bot-a", "timestamp": "2099-01-01T10:00:00Z",
        "entry_hash": "a" * 64, "prev_hash": "0" * 64,
    },
]


@pytest.mark.anyio
async def test_task_detail_open():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=[]), \
         patch("server._fetch_github_issue", return_value=MOCK_ISSUE):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/QUASI-054")

    assert resp.status_code == 200
    data = resp.json()
    assert data["quasi:taskId"] == "QUASI-054"
    assert data["quasi:status"] == "open"
    assert data["quasi:ledgerEntries"] == []
    assert data["task"]["number"] == 54
    assert data["task"]["title"] == "QUASI-054: /tasks/{id} endpoint"
    assert data["task"]["state"] == "open"


@pytest.mark.anyio
async def test_task_detail_plain_number():
    """Plain number route: /tasks/54 same as /tasks/QUASI-054"""
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=[]), \
         patch("server._fetch_github_issue", return_value=MOCK_ISSUE):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/54")

    assert resp.status_code == 200
    assert resp.json()["quasi:taskId"] == "QUASI-054"


@pytest.mark.anyio
async def test_task_detail_with_ledger_entries():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=MOCK_LEDGER), \
         patch("server._fetch_github_issue", return_value=MOCK_ISSUE):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/QUASI-054")

    assert resp.status_code == 200
    data = resp.json()
    assert data["quasi:status"] == "claimed"
    assert len(data["quasi:ledgerEntries"]) == 1
    assert data["quasi:ledgerEntries"][0]["type"] == "claim"
    assert data["quasi:claimedBy"] == "bot-a"


@pytest.mark.anyio
async def test_task_detail_github_unavailable():
    """Falls back gracefully when GitHub is unreachable."""
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server.load_ledger", return_value=[]), \
         patch("server._fetch_github_issue", return_value=None):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/QUASI-054")

    assert resp.status_code == 200
    data = resp.json()
    assert data["quasi:taskId"] == "QUASI-054"
    assert "task" not in data  # no GitHub data — graceful degradation


@pytest.mark.anyio
async def test_task_detail_invalid_id():
    from httpx import ASGITransport, AsyncClient
    from server import app

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        resp = await ac.get("/quasi-board/tasks/INVALID")

    assert resp.status_code == 400
