"""Smoke test for the /quasi-board/health endpoint."""
import importlib

import pytest
from httpx import ASGITransport, AsyncClient


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.mark.anyio
async def test_health_returns_ok(tmp_path, monkeypatch):
    """Health endpoint returns 200 with status ok."""
    monkeypatch.setenv("QUASI_DATA_DIR", str(tmp_path))
    monkeypatch.setenv("QUASI_LEDGER_DIR", str(tmp_path))
    monkeypatch.setenv("QUASI_DOMAIN", "localhost")

    import server
    importlib.reload(server)

    transport = ASGITransport(app=server.app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/quasi-board/health")

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["domain"] == "localhost"
