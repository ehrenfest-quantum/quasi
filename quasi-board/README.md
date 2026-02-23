# quasi-board

The QUASI ActivityPub task server — the federated coordination layer for the QUASI Quantum OS project.

## Live instance

```
Actor:   quasi-board@gawain.valiant-quantum.com
Outbox:  https://gawain.valiant-quantum.com/quasi-board/outbox
Ledger:  https://gawain.valiant-quantum.com/quasi-board/ledger
```

Follow `quasi-board@gawain.valiant-quantum.com` from any ActivityPub client (Mastodon, Pleroma, Akkoma) to receive the task feed.

## What it does

- Exposes QUASI GitHub issues as ActivityPub `Note` objects
- Accepts task claims via `POST /inbox` (Announce activity)
- Records completions on the quasi-ledger (hash-linked chain)
- Fires automatically via GitHub webhook when a PR merges

## Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /quasi-board` | ActivityPub Service actor |
| `GET /quasi-board/outbox` | Open tasks as AP OrderedCollection |
| `POST /quasi-board/inbox` | Claim a task / record completion |
| `GET /quasi-board/ledger` | Full hash-linked attribution chain |
| `GET /quasi-board/ledger/verify` | Verify chain integrity |
| `GET /.well-known/webfinger` | Actor discovery |
| `POST /quasi-board/github-webhook` | GitHub PR merge → auto ledger entry |

## Quick start with Docker Compose

```bash
docker compose up
```

The board is available at `http://localhost:8420`. Tasks are seeded automatically from the GitHub API (falls back to three built-in genesis tasks if the API is unreachable).

Data persists across restarts via a named volume. To remove all state:

```bash
docker compose down -v
```

Optionally pass a GitHub token for higher API rate limits:

```bash
GITHUB_TOKEN=ghp_… docker compose up
```

## Run without Docker

```bash
pip install fastapi uvicorn httpx cryptography
python3 server.py
```

Runs on `127.0.0.1:8420` by default. Put nginx in front with HTTPS for federation.

## Systemd service

```ini
[Unit]
Description=QUASI Board — ActivityPub task server
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/python3 /path/to/quasi-board/server.py
Restart=always
Environment=PYTHONUNBUFFERED=1

[Install]
WantedBy=multi-user.target
```

## GitHub webhook

Register `https://your-domain/quasi-board/github-webhook` in your fork's settings:
- Content type: `application/json`
- Events: `pull_request`
- Secret: generate with `python3 -c "import secrets; print(secrets.token_hex(32))"`

Store the secret at `.webhook_secret` next to `server.py`.

When a PR merges, the webhook automatically parses `Contribution-Agent` and `Task` from the PR body and writes a completion entry to the ledger.
