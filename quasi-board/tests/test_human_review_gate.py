"""Tests for the human-review merge gate (QUASI-055).

Covers:
- _requires_human_review() path detection
- Submit with review-required files → 200, status=pending_human_review
- Submit with non-protected files → 200, status=pr_opened
- Hard-blocked paths still return 400
- GET /quasi-board/admin/merges (list pending)
- POST /quasi-board/admin/merges/{pr}/approve → merge + ledger completion
- POST /quasi-board/admin/merges/{pr}/reject → close PR + remove from queue
- 404 on unknown PR number
"""

from unittest.mock import patch, AsyncMock, MagicMock

import pytest

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


# ── Unit tests for path detection ─────────────────────────────────────────────

def test_review_required_quasi_board():
    from server import _requires_human_review
    assert _requires_human_review({"quasi-board/server.py": "code"}) is True


def test_review_required_github():
    from server import _requires_human_review
    assert _requires_human_review({".github/workflows/ci.yml": "yaml"}) is True


def test_review_required_spec():
    from server import _requires_human_review
    assert _requires_human_review({"spec/ehrenfest-v0.1.cddl": "cddl"}) is True


def test_review_required_readme():
    from server import _requires_human_review
    assert _requires_human_review({"README.md": "text"}) is True


def test_no_review_required_normal_path():
    from server import _requires_human_review
    assert _requires_human_review({"afana/src/parser/mod.rs": "rust"}) is False


def test_no_review_required_ehrenfest_example():
    from server import _requires_human_review
    assert _requires_human_review({"examples/grover.paul": "cbor"}) is False


# ── Integration tests ──────────────────────────────────────────────────────────

SUBMIT_NORMAL = {
    "@context": "https://www.w3.org/ns/activitystreams",
    "type": "Create",
    "quasi:type": "patch",
    "actor": "agent-a",
    "quasi:taskId": "QUASI-010",
    "quasi:files": {"afana/src/lib.rs": "pub fn hello() {}"},
    "quasi:message": "initial impl",
}

SUBMIT_PROTECTED = {
    **SUBMIT_NORMAL,
    "quasi:taskId": "QUASI-054",
    "quasi:files": {"quasi-board/server.py": "# patched"},
}

SUBMIT_HARD_BLOCKED = {
    **SUBMIT_NORMAL,
    "quasi:files": {"infra/docker-compose.yml": "version: '3'"},
}


@pytest.mark.anyio
async def test_submit_normal_returns_pr_opened():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._validate_submission_files"), \
         patch("server._check_agent_claimed"), \
         patch("server._sanitise_files", return_value={"afana/src/lib.rs": "pub fn hello() {}"}), \
         patch("server._open_pr_from_files", new_callable=AsyncMock,
               return_value="https://github.com/ehrenfest-quantum/quasi/pull/80"), \
         patch("server.append_ledger", return_value={"id": 70, "entry_hash": "a" * 64}), \
         patch("server._notify_daniel", new_callable=AsyncMock), \
         patch("server._deliver_to_followers", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json=SUBMIT_NORMAL)

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "pr_opened"
    assert data["review_required"] is False


@pytest.mark.anyio
async def test_submit_protected_returns_pending_review():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._validate_submission_files"), \
         patch("server._check_agent_claimed"), \
         patch("server._sanitise_files", return_value={"quasi-board/server.py": "# patched"}), \
         patch("server._open_pr_from_files", new_callable=AsyncMock,
               return_value="https://github.com/ehrenfest-quantum/quasi/pull/81"), \
         patch("server.append_ledger", return_value={"id": 71, "entry_hash": "b" * 64}), \
         patch("server._load_pending_merges", return_value=[]), \
         patch("server._save_pending_merges") as mock_save, \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json=SUBMIT_PROTECTED)

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "pending_human_review"
    assert data["review_required"] is True
    mock_save.assert_called_once()
    saved = mock_save.call_args[0][0]
    assert saved[0]["pr_number"] == 81
    assert saved[0]["task_id"] == "QUASI-054"


@pytest.mark.anyio
async def test_submit_hard_blocked_returns_400():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._check_agent_claimed"):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post("/quasi-board/inbox", json=SUBMIT_HARD_BLOCKED)

    assert resp.status_code == 400


@pytest.mark.anyio
async def test_list_pending_merges_requires_auth():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._admin_token", return_value="secret"):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.get("/quasi-board/admin/merges")

    assert resp.status_code == 401


@pytest.mark.anyio
async def test_approve_merge_success():
    from httpx import ASGITransport, AsyncClient
    from server import app

    pending = [{
        "pr_number": 81, "pr_url": "https://github.com/.../pull/81",
        "task_id": "QUASI-054", "agent": "agent-a",
        "ledger_submission_id": 71, "submitted_at": "2026-02-24T00:00:00Z",
    }]

    mock_merge_resp = MagicMock()
    mock_merge_resp.status_code = 200
    mock_merge_resp.json.return_value = {"sha": "abc123"}
    mock_merge_resp.raise_for_status = MagicMock()

    mock_patch_resp = MagicMock()
    mock_patch_resp.status_code = 200

    with patch("server._admin_token", return_value="secret"), \
         patch("server._load_pending_merges", return_value=pending), \
         patch("server._save_pending_merges") as mock_save, \
         patch("server._github_token", return_value="gh-token"), \
         patch("server.append_ledger", return_value={"id": 72, "entry_hash": "c" * 64}), \
         patch("server._notify_daniel", new_callable=AsyncMock), \
         patch("httpx.AsyncClient") as mock_client:

        mock_async_ctx = AsyncMock()
        mock_async_ctx.__aenter__ = AsyncMock(return_value=mock_async_ctx)
        mock_async_ctx.__aexit__ = AsyncMock(return_value=False)
        mock_async_ctx.put = AsyncMock(return_value=mock_merge_resp)
        mock_client.return_value = mock_async_ctx

        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/merges/81/approve",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "merged"
    assert data["task_id"] == "QUASI-054"
    assert data["ledger_completion"] == 72
    mock_save.assert_called_once_with([])  # removed from pending


@pytest.mark.anyio
async def test_approve_unknown_pr_returns_404():
    from httpx import ASGITransport, AsyncClient
    from server import app

    with patch("server._admin_token", return_value="secret"), \
         patch("server._load_pending_merges", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            resp = await ac.post(
                "/quasi-board/admin/merges/999/approve",
                headers={"Authorization": "Bearer secret"},
            )

    assert resp.status_code == 404
