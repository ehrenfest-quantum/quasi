from unittest.mock import patch, AsyncMock

import pytest

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

PROPOSE_ACTIVITY = {
    "@context": ["https://www.w3.org/ns/activitystreams", {"quasi": "https://quasi.dev/ns#"}],
    "type": "quasi:Propose",
    "actor": "claude-sonnet-4-6",
    "object": {
        "type": "quasi:TaskProposal",
        "quasi:title": "Add ZX-calculus optimization to Afana compiler",
        "quasi:description": "After Afana v0, add a ZX-calculus rewrite pass using PyZX.",
        "quasi:estimatedEffort": "Medium, ~6h",
        "quasi:affectedComponents": ["afana", "spec"],
        "quasi:successCriteria": ["Gate count reduced by ≥20%", "All existing tests pass"],
        "quasi:rationale": "Reduces gate count by 30-40% on typical circuits",
    },
}


@pytest.mark.anyio
async def test_propose_returns_202():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals") as mock_save, \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json=PROPOSE_ACTIVITY)

    assert resp.status_code == 202
    data = resp.json()
    assert data["status"] == "proposed"
    assert data["id"] == "prop-001"
    mock_save.assert_called_once()
    saved = mock_save.call_args[0][0]
    assert saved[0]["title"] == "Add ZX-calculus optimization to Afana compiler"
    assert saved[0]["status"] == "pending"


@pytest.mark.anyio
async def test_propose_missing_title_returns_400():
    from httpx import ASGITransport, AsyncClient
    from server import app

    bad = {**PROPOSE_ACTIVITY, "object": {"quasi:description": "some desc"}}
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        resp = await ac.post("/quasi-board/inbox", json=bad)

    assert resp.status_code == 400


@pytest.mark.anyio
async def test_get_proposals_empty():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/proposals")

    assert resp.status_code == 200
    data = resp.json()
    assert data["totalItems"] == 0
    assert data["items"] == []


@pytest.mark.anyio
async def test_get_proposals_returns_list():
    from httpx import ASGITransport, AsyncClient
    from server import app

    stored = [{"id": "prop-001", "title": "T", "status": "pending"}]
    with patch("server._load_proposals", return_value=stored):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/proposals")

    assert resp.status_code == 200
    assert resp.json()["totalItems"] == 1


@pytest.mark.anyio
async def test_accept_proposal_requires_auth():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._admin_token", return_value="secret"):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/admin/proposals/prop-001/accept")

    assert resp.status_code == 401


@pytest.mark.anyio
async def test_accept_proposal_success():
    from httpx import ASGITransport, AsyncClient
    from server import app

    pending = [{"id": "prop-001", "title": "T", "description": "D",
                "proposed_by": "bot", "status": "pending"}]

    with (
        patch("server._admin_token", return_value="secret"),
        patch("server._load_proposals", return_value=pending),
        patch(
            "server._create_github_issue_for_proposal",
            return_value={
                "number": 42,
                "html_url": "https://github.com/ehrenfest-quantum/quasi/issues/42",
            },
        ),
        patch("server._save_proposals"),
        patch("server.append_ledger", return_value={"id": 99, "entry_hash": "a" * 64}),
        patch("server._notify_daniel", new_callable=AsyncMock),
    ):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/proposals/prop-001/accept",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "accepted"
    assert data["proposal"]["status"] == "accepted"
    assert data["proposal"]["task_issue_number"] == 42
    assert data["task"]["number"] == 42
    assert data["task"]["url"].endswith("/42")
    assert data["ledger_entry"] == 99


@pytest.mark.anyio
async def test_reject_proposal_success():
    from httpx import ASGITransport, AsyncClient
    from server import app

    pending = [{"id": "prop-001", "title": "T", "description": "D",
                "proposed_by": "bot", "status": "pending"}]

    with patch("server._admin_token", return_value="secret"), \
         patch("server._load_proposals", return_value=pending), \
         patch("server._save_proposals"):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/proposals/prop-001/reject",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 200
    assert resp.json()["status"] == "rejected"


@pytest.mark.anyio
async def test_accept_nonexistent_proposal_returns_404():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._admin_token", return_value="secret"), \
         patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/proposals/prop-999/accept",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 404


@pytest.mark.anyio
async def test_accept_already_accepted_returns_409():
    from httpx import ASGITransport, AsyncClient
    from server import app

    already = [{"id": "prop-001", "title": "T", "description": "D",
                "proposed_by": "bot", "status": "accepted"}]

    with patch("server._admin_token", return_value="secret"), \
         patch("server._load_proposals", return_value=already):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/proposals/prop-001/accept",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 409
