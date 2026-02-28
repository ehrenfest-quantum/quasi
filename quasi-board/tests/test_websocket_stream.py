"""Tests for the /quasi-board/stream WebSocket endpoint (QUASI-029)."""

import importlib
import json

import pytest


@pytest.fixture
def anyio_backend():
    return "asyncio"


def _make_server(monkeypatch, tmp_path):
    """Reload server module with test environment."""
    monkeypatch.setenv("QUASI_DATA_DIR", str(tmp_path))
    monkeypatch.setenv("QUASI_LEDGER_DIR", str(tmp_path))
    monkeypatch.setenv("QUASI_DOMAIN", "localhost")
    import server
    importlib.reload(server)
    return server


# ---------------------------------------------------------------------------
# _StreamManager unit tests
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_stream_manager_starts_empty(monkeypatch, tmp_path):
    """New _StreamManager has no connections."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()
    assert len(manager._connections) == 0


@pytest.mark.anyio
async def test_stream_manager_connect_adds_ws(monkeypatch, tmp_path):
    """connect() accepts and registers the WebSocket."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()

    class FakeWS:
        accepted = False

        async def accept(self):
            self.accepted = True

        async def send_text(self, text):
            pass

    ws = FakeWS()
    await manager.connect(ws)
    assert ws.accepted
    assert ws in manager._connections


@pytest.mark.anyio
async def test_stream_manager_disconnect_removes_ws(monkeypatch, tmp_path):
    """disconnect() removes the WebSocket from the active set."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()

    class FakeWS:
        async def accept(self):
            pass

        async def send_text(self, text):
            pass

    ws = FakeWS()
    await manager.connect(ws)
    assert ws in manager._connections
    manager.disconnect(ws)
    assert ws not in manager._connections


@pytest.mark.anyio
async def test_stream_manager_disconnect_idempotent(monkeypatch, tmp_path):
    """disconnect() on a non-connected WebSocket does not raise."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()

    class FakeWS:
        async def accept(self):
            pass

    ws = FakeWS()
    manager.disconnect(ws)  # should not raise


@pytest.mark.anyio
async def test_stream_manager_broadcast_delivers_to_one_client(monkeypatch, tmp_path):
    """broadcast() sends JSON payload to connected WebSocket."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()
    received = []

    class FakeWS:
        async def accept(self):
            pass

        async def send_text(self, text):
            received.append(json.loads(text))

    ws = FakeWS()
    await manager.connect(ws)
    await manager.broadcast({"type": "task_claimed", "task": {"id": "T1"}})
    assert len(received) == 1
    assert received[0]["type"] == "task_claimed"
    assert received[0]["task"]["id"] == "T1"


@pytest.mark.anyio
async def test_stream_manager_broadcast_delivers_to_multiple_clients(monkeypatch, tmp_path):
    """broadcast() reaches all connected clients."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()
    received_a = []
    received_b = []

    class FakeWSA:
        async def accept(self):
            pass

        async def send_text(self, text):
            received_a.append(json.loads(text))

    class FakeWSB:
        async def accept(self):
            pass

        async def send_text(self, text):
            received_b.append(json.loads(text))

    await manager.connect(FakeWSA())
    await manager.connect(FakeWSB())
    await manager.broadcast({"type": "new_task", "task": {"id": "T2"}})
    assert len(received_a) == 1
    assert len(received_b) == 1
    assert received_a[0] == received_b[0]


@pytest.mark.anyio
async def test_stream_manager_broadcast_removes_dead_clients(monkeypatch, tmp_path):
    """broadcast() silently removes clients that raise on send."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()

    class DeadWS:
        async def accept(self):
            pass

        async def send_text(self, text):
            raise RuntimeError("connection closed")

    ws = DeadWS()
    await manager.connect(ws)
    assert ws in manager._connections

    await manager.broadcast({"type": "ping"})
    assert ws not in manager._connections


@pytest.mark.anyio
async def test_stream_manager_broadcast_empty_no_error(monkeypatch, tmp_path):
    """broadcast() with no connected clients does not raise."""
    server = _make_server(monkeypatch, tmp_path)
    manager = server._StreamManager()
    await manager.broadcast({"type": "ping"})  # should not raise


# ---------------------------------------------------------------------------
# _broadcast_event wrapper
# ---------------------------------------------------------------------------


@pytest.mark.anyio
async def test_broadcast_event_wraps_in_envelope(monkeypatch, tmp_path):
    """_broadcast_event sends {"type": ..., "task": ...} envelope."""
    server = _make_server(monkeypatch, tmp_path)
    received = []

    async def fake_broadcast(event):
        received.append(event)

    server._stream.broadcast = fake_broadcast
    await server._broadcast_event("task_claimed", {"id": "QUASI-042", "agent": "bot"})
    assert len(received) == 1
    assert received[0] == {"type": "task_claimed", "task": {"id": "QUASI-042", "agent": "bot"}}


@pytest.mark.anyio
async def test_broadcast_event_new_task(monkeypatch, tmp_path):
    server = _make_server(monkeypatch, tmp_path)
    received = []

    async def fake_broadcast(event):
        received.append(event)

    server._stream.broadcast = fake_broadcast
    await server._broadcast_event("new_task", {"id": "P1", "title": "New feature"})
    assert received[0]["type"] == "new_task"
    assert received[0]["task"]["title"] == "New feature"


@pytest.mark.anyio
async def test_broadcast_event_task_completed(monkeypatch, tmp_path):
    server = _make_server(monkeypatch, tmp_path)
    received = []

    async def fake_broadcast(event):
        received.append(event)

    server._stream.broadcast = fake_broadcast
    await server._broadcast_event("task_completed", {"id": "QUASI-001", "pr_url": "https://gh/1"})
    assert received[0]["type"] == "task_completed"
    assert received[0]["task"]["pr_url"] == "https://gh/1"


@pytest.mark.anyio
async def test_broadcast_event_task_expired(monkeypatch, tmp_path):
    server = _make_server(monkeypatch, tmp_path)
    received = []

    async def fake_broadcast(event):
        received.append(event)

    server._stream.broadcast = fake_broadcast
    await server._broadcast_event("task_expired", {"id": "QUASI-005"})
    assert received[0]["type"] == "task_expired"


# ---------------------------------------------------------------------------
# Endpoint exists (smoke test — just verify route is registered)
# ---------------------------------------------------------------------------


def test_stream_route_registered(monkeypatch, tmp_path):
    """The /quasi-board/stream WebSocket route exists in the app."""
    server = _make_server(monkeypatch, tmp_path)
    routes = {r.path for r in server.app.routes}
    assert "/quasi-board/stream" in routes
