import hashlib
import json
import pytest
from datetime import datetime, timezone, timedelta
from unittest.mock import patch

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from server import _check_agent_claimed


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


def test_valid_claim_passes():
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", hours_ago=1)]
    with patch("server.load_ledger", return_value=chain):
        _check_agent_claimed("QUASI-001", "agent-a")  # should not raise


def test_expired_claim_raises_403():
    chain = [_make_entry(1, "claim", "QUASI-001", "agent-a", hours_ago=25)]
    with patch("server.load_ledger", return_value=chain):
        with pytest.raises(Exception) as exc_info:
            _check_agent_claimed("QUASI-001", "agent-a")
        assert exc_info.value.status_code == 403
        assert "expired" in str(exc_info.value.detail).lower()


def test_no_claim_raises_403():
    with patch("server.load_ledger", return_value=[]):
        with pytest.raises(Exception) as exc_info:
            _check_agent_claimed("QUASI-001", "agent-a")
        assert exc_info.value.status_code == 403
