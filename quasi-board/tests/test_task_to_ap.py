from datetime import datetime, timezone, timedelta
from unittest.mock import patch

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from server import task_to_ap


def _gh_task(number=1, title="Test Task", url="https://github.com/test/1"):
    return {"number": number, "title": title, "html_url": url, "body": "desc"}


def test_open_task_has_open_status():
    with patch("server._effective_task_status", return_value={"status": "open"}):
        activity = task_to_ap(_gh_task())
    note = activity["object"]
    assert note["quasi:status"] == "open"
    assert "quasi:claimedBy" not in note


def test_claimed_task_shows_agent_and_expiry():
    expires = (datetime.now(timezone.utc) + timedelta(hours=20)).isoformat()
    with patch("server._effective_task_status", return_value={
        "status": "claimed", "agent": "bot-x", "expires_at": expires,
    }):
        activity = task_to_ap(_gh_task())
    note = activity["object"]
    assert note["quasi:status"] == "claimed"
    assert note["quasi:claimedBy"] == "bot-x"
    assert note["quasi:expiresAt"] == expires


def test_done_task_has_done_status():
    with patch("server._effective_task_status", return_value={"status": "done"}):
        activity = task_to_ap(_gh_task())
    note = activity["object"]
    assert note["quasi:status"] == "done"
