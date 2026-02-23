import hashlib
import json
from datetime import datetime, timezone, timedelta
from unittest.mock import patch

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from server import _effective_task_status, CLAIM_TTL_MINUTES  # noqa: F401


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
