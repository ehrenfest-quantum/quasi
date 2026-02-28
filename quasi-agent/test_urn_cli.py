from __future__ import annotations

import importlib.util
import io
import json
import threading
from contextlib import redirect_stdout
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def _load_cli_module():
    path = Path(__file__).with_name("cli.py")
    spec = importlib.util.spec_from_file_location("quasi_agent_cli", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def _start_server():
    state = {
        "published": [],
        "artifact": bytes.fromhex("a0"),
    }

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            if self.path != "/urns":
                self.send_response(404)
                self.end_headers()
                return
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length))
            state["published"].append(body)
            payload = {
                "name": body["name"],
                "version": body["version"],
                "download_url": f"/urns/{body['name']}/{body['version']}/download",
            }
            data = json.dumps(payload).encode("utf-8")
            self.send_response(201)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path.startswith("/urns/search?q="):
                data = json.dumps(
                    {
                        "items": [
                            {
                                "name": "grover-search",
                                "latest_version": "0.1.0",
                                "description": "Grover search primitive",
                            }
                        ]
                    }
                ).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            if self.path == "/urns/grover-search":
                data = json.dumps(
                    {
                        "name": "grover-search",
                        "latest_version": "0.1.0",
                        "versions": [{"version": "0.1.0"}],
                    }
                ).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            if self.path == "/urns/grover-search/0.1.0":
                data = json.dumps({"name": "grover-search", "version": "0.1.0"}).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            if self.path == "/urns/grover-search/0.1.0/download":
                self.send_response(200)
                self.send_header("Content-Type", "application/cbor")
                self.send_header("Content-Length", str(len(state["artifact"])))
                self.end_headers()
                self.wfile.write(state["artifact"])
                return
            self.send_response(404)
            self.end_headers()

        def log_message(self, *_args):
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, state


def test_urn_publish_search_and_install(tmp_path):
    cli = _load_cli_module()
    server, state = _start_server()
    base = f"http://127.0.0.1:{server.server_address[1]}"

    urn_file = tmp_path / "grover.urn"
    urn_file.write_text(
        json.dumps(
            {
                "name": "grover-search",
                "version": "0.1.0",
                "description": "Grover search primitive",
                "urn_schema": "quasi.urn.v1",
                "entrypoint": "main",
                "program_cbor_hex": "a0",
            }
        ),
        encoding="utf-8",
    )
    out_file = tmp_path / "downloaded.cbor"

    try:
        publish_stdout = io.StringIO()
        with redirect_stdout(publish_stdout):
            cli.cmd_urn_publish(base, str(urn_file))
        assert state["published"][0]["name"] == "grover-search"
        assert "Published grover-search@0.1.0" in publish_stdout.getvalue()

        search_stdout = io.StringIO()
        with redirect_stdout(search_stdout):
            cli.cmd_urn_search(base, "grover")
        assert "grover-search" in search_stdout.getvalue()

        install_stdout = io.StringIO()
        with redirect_stdout(install_stdout):
            cli.cmd_urn_install(base, "grover-search", output=str(out_file))
        assert out_file.read_bytes() == state["artifact"]
        assert "Installed grover-search@0.1.0" in install_stdout.getvalue()
    finally:
        server.shutdown()
        server.server_close()
