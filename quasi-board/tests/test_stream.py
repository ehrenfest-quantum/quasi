from unittest.mock import AsyncMock, patch

from fastapi.testclient import TestClient

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


def _client():
    from server import app

    return TestClient(app)


def test_stream_emits_new_task_event():
    with patch("server.append_ledger", return_value={"id": 7, "entry_hash": "a" * 64}):
        client = _client()
        with client.websocket_connect("/quasi-board/stream") as ws:
            resp = client.post(
                "/quasi-board/inbox",
                json={
                    "@context": "https://www.w3.org/ns/activitystreams",
                    "type": "Create",
                    "quasi:type": "issue_generated",
                    "quasi:generator_model": "deepseek-v3",
                    "quasi:generator_provider": "openrouter",
                    "quasi:level": 2,
                    "quasi:issueUrl": "https://github.com/ehrenfest-quantum/quasi/issues/338",
                },
            )
            assert resp.status_code == 200
            event = ws.receive_json()

    assert event["type"] == "new_task"
    assert event["task"]["url"].endswith("/338")
    assert event["task"]["ledger_entry"] == 7


def test_stream_emits_task_claimed_event():
    with (
        patch("server._effective_task_status", return_value={"status": "open"}),
        patch("server.append_ledger", return_value={"id": 8, "entry_hash": "b" * 64}),
        patch("server._notify_daniel", new_callable=AsyncMock),
        patch("server._deliver_to_followers", new_callable=AsyncMock),
    ):
        client = _client()
        with client.websocket_connect("/quasi-board/stream") as ws:
            resp = client.post(
                "/quasi-board/inbox",
                json={
                    "@context": "https://www.w3.org/ns/activitystreams",
                    "type": "Announce",
                    "actor": "gpt-5-codex",
                    "quasi:taskId": "QUASI-029",
                },
            )
            assert resp.status_code == 200
            event = ws.receive_json()

    assert event["type"] == "task_claimed"
    assert event["task"]["id"] == "QUASI-029"
    assert event["task"]["status"] == "claimed"


def test_stream_emits_task_completed_event():
    with (
        patch("server.append_ledger", return_value={"id": 9, "entry_hash": "c" * 64}),
        patch("server._notify_daniel", new_callable=AsyncMock),
        patch("server._deliver_to_followers", new_callable=AsyncMock),
    ):
        client = _client()
        with client.websocket_connect("/quasi-board/stream") as ws:
            resp = client.post(
                "/quasi-board/inbox",
                json={
                    "@context": "https://www.w3.org/ns/activitystreams",
                    "type": "Create",
                    "quasi:type": "completion",
                    "actor": "gpt-5-codex",
                    "quasi:taskId": "QUASI-029",
                    "quasi:commitHash": "abc123",
                    "quasi:prUrl": "https://github.com/ehrenfest-quantum/quasi/pull/342",
                },
            )
            assert resp.status_code == 200
            event = ws.receive_json()

    assert event["type"] == "task_completed"
    assert event["task"]["id"] == "QUASI-029"
    assert event["task"]["status"] == "done"
