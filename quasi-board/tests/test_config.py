"""Tests for server.py configuration via environment variables.

Since ``server.py`` reads env vars and calls ``_load_or_create_keys()`` at
module load time, we must:
1. Point ``QUASI_DATA_DIR`` at a writable temp directory during every reload
   so the key-generation code can create directories/files.
2. Set/unset other env vars *before* reloading.

The test for default data-dir paths verifies the default value by checking
what ``os.environ.get`` would return when the env var is absent, since
actually loading the module with the production default (``/home/vops/…``)
would fail on a development machine.
"""

import importlib
import sys
from pathlib import Path

import pytest


def _reload_server(monkeypatch, tmp_path, env_overrides: dict | None = None):
    """Set env vars, then reload ``server`` with a writable data directory.

    ``QUASI_DATA_DIR`` is always pointed at *tmp_path* unless the caller
    explicitly overrides it, so the key-generation code at module load time
    can create directories and files without touching production paths.
    """
    # Clear env vars so defaults take effect (except DATA_DIR, see below)
    for var in ("QUASI_DOMAIN", "QUASI_DATA_DIR", "QUASI_LEDGER_DIR", "QUASI_GITHUB_REPO"):
        monkeypatch.delenv(var, raising=False)

    # Always provide a writable data dir unless the caller sets one
    if env_overrides is None or "QUASI_DATA_DIR" not in env_overrides:
        monkeypatch.setenv("QUASI_DATA_DIR", str(tmp_path))

    # Apply requested overrides
    if env_overrides:
        for key, value in env_overrides.items():
            monkeypatch.setenv(key, value)

    srv = sys.modules.get("server")
    if srv is None:
        import server as srv  # noqa: F811
    else:
        importlib.reload(srv)

    return srv


# ── Default values (no env vars set) ─────────────────────────────────────────


class TestDefaults:
    """Verify that defaults match the previously hardcoded production values."""

    def test_default_domain(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path)
        assert srv.DOMAIN == "gawain.valiant-quantum.com"

    def test_default_actor_url(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path)
        assert srv.ACTOR_URL == "https://gawain.valiant-quantum.com/quasi-board"

    def test_default_github_repo(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path)
        assert srv.GITHUB_REPO == "ehrenfest-quantum/quasi"

    def test_default_ledger_file(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path)
        assert srv.LEDGER_FILE == Path("/home/vops/quasi-ledger/ledger.json")

    def test_data_dir_derives_all_data_paths(self, monkeypatch, tmp_path):
        """QUASI_DATA_DIR controls ACTOR_KEY_FILE, FOLLOWERS_FILE, etc.

        We cannot reload with the *default* data dir (``/home/vops/…`` does
        not exist on dev machines), but the override test in
        ``TestDataDirOverride`` proves the wiring.  Here we verify that
        a known DATA_DIR produces the expected derived paths -- confirming
        the path-construction logic is correct.
        """
        srv = _reload_server(monkeypatch, tmp_path)
        data_dir = srv._DATA_DIR
        assert srv.ACTOR_KEY_FILE == data_dir / "keys" / "actor.pem"
        assert srv.FOLLOWERS_FILE == data_dir / "followers.json"
        assert srv.GITHUB_TOKEN_FILE == data_dir / ".github_token"
        assert srv.MATRIX_CREDS_FILE == data_dir / "matrix_credentials.json"
        assert srv.WEBHOOK_SECRET_FILE == data_dir / ".webhook_secret"

    def test_default_data_dir_in_source(self):
        """The hardcoded default for QUASI_DATA_DIR in server.py source is
        ``/home/vops/quasi-board`` -- matching the previous production value.
        """
        import inspect
        srv = sys.modules["server"]
        source = inspect.getsource(srv)
        assert 'os.environ.get("QUASI_DATA_DIR", "/home/vops/quasi-board")' in source


# ── Environment variable overrides ───────────────────────────────────────────


class TestDomainOverride:
    """QUASI_DOMAIN should change domain and all derived URLs."""

    def test_domain_override(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path, {"QUASI_DOMAIN": "example.org"})
        assert srv.DOMAIN == "example.org"
        assert srv.ACTOR_URL == "https://example.org/quasi-board"
        assert srv.OUTBOX_URL == "https://example.org/quasi-board/outbox"
        assert srv.INBOX_URL == "https://example.org/quasi-board/inbox"
        assert srv.ACTOR_KEY_ID == "https://example.org/quasi-board#main-key"


class TestDataDirOverride:
    """QUASI_DATA_DIR should change all data file paths."""

    def test_data_dir_override(self, monkeypatch, tmp_path):
        custom_dir = tmp_path / "custom-data"
        custom_dir.mkdir()
        srv = _reload_server(monkeypatch, tmp_path, {"QUASI_DATA_DIR": str(custom_dir)})
        assert srv.ACTOR_KEY_FILE == custom_dir / "keys" / "actor.pem"
        assert srv.FOLLOWERS_FILE == custom_dir / "followers.json"
        assert srv.GITHUB_TOKEN_FILE == custom_dir / ".github_token"
        assert srv.MATRIX_CREDS_FILE == custom_dir / "matrix_credentials.json"
        assert srv.WEBHOOK_SECRET_FILE == custom_dir / ".webhook_secret"


class TestLedgerDirOverride:
    """QUASI_LEDGER_DIR should change the ledger file path."""

    def test_ledger_dir_override(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path, {"QUASI_LEDGER_DIR": "/app/ledger"})
        assert srv.LEDGER_FILE == Path("/app/ledger/ledger.json")

    def test_ledger_dir_independent_of_data_dir(self, monkeypatch, tmp_path):
        """Ledger dir and data dir are independent settings."""
        custom_dir = tmp_path / "custom-data"
        custom_dir.mkdir()
        srv = _reload_server(monkeypatch, tmp_path, {
            "QUASI_DATA_DIR": str(custom_dir),
            "QUASI_LEDGER_DIR": "/app/ledger",
        })
        assert srv.LEDGER_FILE == Path("/app/ledger/ledger.json")
        assert srv.GITHUB_TOKEN_FILE == custom_dir / ".github_token"


class TestGithubRepoOverride:
    """QUASI_GITHUB_REPO should override the repository name."""

    def test_github_repo_override(self, monkeypatch, tmp_path):
        srv = _reload_server(monkeypatch, tmp_path, {"QUASI_GITHUB_REPO": "myorg/myrepo"})
        assert srv.GITHUB_REPO == "myorg/myrepo"
