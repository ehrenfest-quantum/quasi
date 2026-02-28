import hashlib
import json
import os
import sys
from datetime import datetime, timezone, timedelta
from unittest.mock import patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from server import _effective_task_status, _expire_stale_claims  # noqa: E402


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


def test_no_entries_returns_open():
    with patch("server.load_ledger", return_value=[]):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "open"


def test_fresh_claim_returns_claimed():
    # Claimed 5 minutes ago — well within 30-minute TTL
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=5)]
    with patch("server.load_ledger", return_value=chain):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "claimed"
    assert status["agent"] == "agent-a"
    assert "expires_at" in status


def test_expired_claim_returns_open():
    # Claimed 60 minutes ago — past 30-minute TTL
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=60)]
    with patch("server.load_ledger", return_value=chain):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "open"


def test_completion_returns_done():
    chain = [
        _make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=20),
        _make_entry(2, "completion", "QUASI-001", "agent-a", minutes_ago=5),
    ]
    with patch("server.load_ledger", return_value=chain):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "done"


def test_different_task_ignored():
    chain = [_make_entry(1, "claim", "QUASI-002", "agent-a", minutes_ago=5)]
    with patch("server.load_ledger", return_value=chain):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "open"


def test_submission_after_claim_counts_as_active():
    chain = [
        _make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=25),
        _make_entry(2, "submission", "QUASI-001", "agent-a", minutes_ago=5),
    ]
    with patch("server.load_ledger", return_value=chain):
        status = _effective_task_status("QUASI-001")
    assert status["status"] == "claimed"


# ── _expire_stale_claims tests ─────────────────────────────────────────────────

def test_expire_stale_claims_writes_expiry_entry():
    """Expired claim (past TTL) should produce an expiry ledger entry."""
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=60)]
    written = []
    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", side_effect=written.append):
        expired = _expire_stale_claims()
    assert "QUASI-001" in expired
    assert len(written) == 1
    entry = written[0]
    assert entry["type"] == "expiry"
    assert entry["task"] == "QUASI-001"
    assert entry["contributor_agent"] == "agent-a"


def test_expire_stale_claims_skips_active_claims():
    """Fresh claim within TTL should not be expired."""
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=5)]
    written = []
    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", side_effect=written.append):
        expired = _expire_stale_claims()
    assert expired == []
    assert written == []


def test_expire_stale_claims_skips_completed_tasks():
    """Completed tasks should never receive an expiry entry."""
    chain = [
        _make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=60),
        _make_entry(2, "completion", "QUASI-001", "agent-a", minutes_ago=5),
    ]
    written = []
    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", side_effect=written.append):
        expired = _expire_stale_claims()
    assert expired == []
    assert written == []


def test_expire_stale_claims_skips_already_expired():
    """Task with an expiry entry after the last claim must not get a duplicate."""
    claim = _make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=90)
    expiry = _make_entry(2, "expiry", "QUASI-001", "agent-a", minutes_ago=30)
    expiry["id"] = 2  # ensure id > claim id (1)
    chain = [claim, expiry]
    written = []
    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", side_effect=written.append):
        expired = _expire_stale_claims()
    assert expired == []
    assert written == []


def test_expire_stale_claims_multiple_tasks():
    """Only tasks past TTL are expired; active ones are left alone."""
    chain = [
        _make_entry(1, "claim", "QUASI-001", "agent-a", minutes_ago=60),  # stale
        _make_entry(2, "claim", "QUASI-002", "agent-b", minutes_ago=5),   # active
    ]
    written = []
    with patch("server.load_ledger", return_value=chain), \
         patch("server.append_ledger", side_effect=written.append):
        expired = _expire_stale_claims()
    assert expired == ["QUASI-001"]
    assert len(written) == 1
    assert written[0]["task"] == "QUASI-001"
