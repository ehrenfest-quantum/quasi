"""Tests for the Pauli-Test complexity gate on task proposals (issue #85)."""

from unittest.mock import AsyncMock, patch

import pytest
from httpx import ASGITransport, AsyncClient

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


def _propose_body(
    title="Add ZX-calculus optimization Afana compiler",
    description="Detailed description of the proposed work.",
    effort="medium",
    components=None,
    criteria=None,
    level="L1",
    rationale="Reduces gate count by 30%",
):
    return {
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "quasi:Propose",
        "actor": "https://agent.example.com/actor",
        "object": {
            "type": "quasi:TaskProposal",
            "quasi:title": title,
            "quasi:description": description,
            "quasi:estimatedEffort": effort,
            "quasi:affectedComponents": components if components is not None else ["afana", "spec"],
            "quasi:successCriteria": criteria if criteria is not None else ["Tests pass", "Benchmark improves"],
            "quasi:rationale": rationale,
            "quasi:level": level,
        },
    }


# ---------------------------------------------------------------------------
# Valid proposal — should be accepted (202)
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_valid_proposal_accepted():
    """A well-formed medium-effort proposal is accepted with 202."""
    from server import app

    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=_propose_body())
    assert resp.status_code == 202
    assert resp.json()["status"] == "proposed"


@pytest.mark.anyio
async def test_effort_phrase_accepted():
    """Effort phrases like 'Medium, ~6h' are accepted (backward-compat)."""
    from server import app

    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=_propose_body(effort="Medium, ~6h"))
    assert resp.status_code == 202


# ---------------------------------------------------------------------------
# Missing required fields
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_missing_effort_rejected():
    from server import app

    body = _propose_body(effort="")
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400
    assert "estimatedEffort" in resp.json()["detail"]


@pytest.mark.anyio
async def test_invalid_effort_rejected():
    from server import app

    body = _propose_body(effort="easy")
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400


@pytest.mark.anyio
async def test_missing_components_rejected():
    from server import app

    body = _propose_body(components=[])
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400
    assert "affectedComponents" in resp.json()["detail"]


@pytest.mark.anyio
async def test_missing_criteria_rejected():
    from server import app

    body = _propose_body(criteria=[])
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400
    assert "successCriteria" in resp.json()["detail"]


# ---------------------------------------------------------------------------
# Trivial effort gate
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_trivial_effort_rejected():
    """trivial effort proposals are always rejected."""
    from server import app

    body = _propose_body(effort="trivial")
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400
    assert "trivial" in resp.json()["detail"].lower()


# ---------------------------------------------------------------------------
# Small effort scope check
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_small_effort_one_component_two_criteria_rejected():
    """small effort with 1 component and 2 criteria fails the scope check."""
    from server import app

    body = _propose_body(effort="small", components=["afana"], criteria=["Tests pass", "Lints clean"])
    with patch("server._load_proposals", return_value=[]):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 400


@pytest.mark.anyio
async def test_small_effort_two_components_passes():
    """small effort with ≥2 components is accepted."""
    from server import app

    body = _propose_body(effort="small", components=["afana", "spec"], criteria=["Tests pass"])
    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 202


@pytest.mark.anyio
async def test_small_effort_three_criteria_passes():
    """small effort with ≥3 criteria is accepted."""
    from server import app

    body = _propose_body(
        effort="small", components=["afana"],
        criteria=["Tests pass", "Lints clean", "Benchmark improves"],
    )
    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 202


# ---------------------------------------------------------------------------
# L0 global cap
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_l0_cap_enforced():
    """Third L0 proposal is rejected with 429 when cap of 2 is reached."""
    from server import app

    existing_l0 = [
        {"id": "prop-001", "title": "L0 task A infra bootstrap", "status": "pending", "level": "L0"},
        {"id": "prop-002", "title": "L0 task B infra setup", "status": "pending", "level": "L0"},
    ]
    body = _propose_body(
        title="L0 infrastructure something new bootstrap",
        level="L0",
        components=["quasi-board", "spec"],
        effort="medium",
    )
    with patch("server._load_proposals", return_value=existing_l0):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 429


@pytest.mark.anyio
async def test_l0_cap_not_applied_to_l1():
    """L0 cap does not affect L1 proposals."""
    from server import app

    existing_l0 = [
        {"id": "prop-001", "title": "L0 task A bootstrap infra", "status": "pending", "level": "L0"},
        {"id": "prop-002", "title": "L0 task B infra setup", "status": "pending", "level": "L0"},
    ]
    body = _propose_body(
        title="Implement noise model Ehrenfest CBOR channels spec",
        level="L1",
    )
    with patch("server._load_proposals", return_value=existing_l0), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 202


# ---------------------------------------------------------------------------
# Near-duplicate detection
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_duplicate_proposal_rejected():
    """A proposal with >60% keyword overlap with an existing pending one returns 409."""
    from server import app

    existing = [
        {
            "id": "prop-001",
            "title": "Implement ZX-calculus optimization pass Afana compiler gates",
            "status": "pending",
        }
    ]
    body = _propose_body(title="Implement ZX-calculus optimization pass Afana compiler gates redux")
    with patch("server._load_proposals", return_value=existing):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 409
    assert "duplicate" in resp.json()["detail"].lower()


@pytest.mark.anyio
async def test_distinct_proposal_not_flagged():
    """A proposal with distinct title words is not flagged as duplicate."""
    from server import app

    existing = [
        {"id": "prop-001", "title": "WebSocket streaming real-time dashboard feed", "status": "pending"},
    ]
    body = _propose_body(title="Implement noise model Ehrenfest CBOR decoherence channels")
    with patch("server._load_proposals", return_value=existing), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 202


@pytest.mark.anyio
async def test_accepted_proposal_not_checked_for_dedup():
    """Accepted/rejected proposals are not considered for duplicate check."""
    from server import app

    existing = [
        {
            "id": "prop-001",
            "title": "Implement ZX-calculus optimization pass Afana compiler gates",
            "status": "accepted",
        }
    ]
    # Same title as existing accepted proposal — should still pass
    body = _propose_body(title="Implement ZX-calculus optimization pass Afana compiler gates")
    with patch("server._load_proposals", return_value=existing), \
         patch("server._save_proposals"), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.post("/quasi-board/inbox", json=body)
    assert resp.status_code == 202


# ---------------------------------------------------------------------------
# Stored fields
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_accepted_proposal_stores_new_fields():
    """Accepted proposal stores affected_components and success_criteria."""
    from server import app

    saved_proposals = []

    def fake_save(proposals):
        saved_proposals.extend(proposals)

    body = _propose_body(
        components=["afana", "spec"],
        criteria=["All tests pass", "Gate count reduced"],
    )
    with patch("server._load_proposals", return_value=[]), \
         patch("server._save_proposals", side_effect=fake_save), \
         patch("server._notify_daniel", new_callable=AsyncMock):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            await client.post("/quasi-board/inbox", json=body)

    assert len(saved_proposals) == 1
    proposal = saved_proposals[0]
    assert "afana" in proposal["affected_components"]
    assert "All tests pass" in proposal["success_criteria"]
