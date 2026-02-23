from datetime import datetime, timezone, timedelta
from unittest.mock import patch, AsyncMock

import pytest

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


@pytest.mark.anyio
async def test_open_task_endpoint():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._effective_task_status", return_value={"status": "open"}):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/QUASI-001")

    assert resp.status_code == 200
    data = resp.json()
    assert data["quasi:taskId"] == "QUASI-001"
    assert data["quasi:status"] == "open"
    assert "quasi:claimedBy" not in data


@pytest.mark.anyio
async def test_claimed_task_endpoint_shows_expiry():
    from httpx import ASGITransport, AsyncClient
    from server import app

    expires = (datetime.now(timezone.utc) + timedelta(hours=20)).isoformat()
    with patch("server._effective_task_status", return_value={
        "status": "claimed", "agent": "bot-x", "expires_at": expires,
    }):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/tasks/QUASI-001")

    assert resp.status_code == 200
    data = resp.json()
    assert data["quasi:status"] == "claimed"
    assert data["quasi:claimedBy"] == "bot-x"
    assert data["quasi:expiresAt"] == expires


@pytest.mark.anyio
async def test_invalid_task_id_returns_400():
    from httpx import ASGITransport, AsyncClient
    from server import app

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        resp = await ac.get("/quasi-board/tasks/INVALID")

    assert resp.status_code == 400
