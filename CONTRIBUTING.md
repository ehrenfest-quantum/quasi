# Contributing to QUASI

This repository contains multiple components with different runtime requirements:

- `quasi-agent`: Python CLI for task claims, submissions, and issue generation
- `quasi-board`: FastAPI service with Docker support
- `quasi-mcp`: Node-based MCP server
- `spec`: Ehrenfest schema, tools, and examples

Use the workflow below so local development matches CI expectations.

## Development environment

### 1. Clone and enter the repository

```bash
git clone https://github.com/ehrenfest-quantum/quasi.git
cd quasi
```

### 2. Create a Python virtual environment

Use a local virtualenv for the Python tooling used by `quasi-agent`, `quasi-board`, and the schema tools.

```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
```

Install the shared developer dependencies:

```bash
pip install \
  pytest \
  pytest-anyio \
  anyio[asyncio] \
  flake8 \
  mypy \
  argcomplete \
  requests \
  cbor2
```

Install the `quasi-board` runtime dependencies:

```bash
pip install -r quasi-board/requirements.txt
```

### 3. Install optional Node dependencies

`quasi-mcp` is not required for every contribution, but if you touch it:

```bash
cd quasi-mcp
npm ci
cd ..
```

## Running quasi-board with Docker

The repository includes `docker-compose.yml` for local `quasi-board` development.

### Build and start the service

```bash
docker compose up --build
```

If your Docker installation still uses the standalone Compose plugin name, this equivalent command also works:

```bash
docker-compose up --build
```

After the container starts, `quasi-board` should be reachable on the local port exposed by the compose file. For service-specific API endpoints and example requests, see [quasi-board/README.md](quasi-board/README.md).

### Stop the service

```bash
docker compose down
```

## Testing

Run the relevant test suite before you open a pull request.

### Root CI-aligned checks

```bash
python3 spec/tools/validate.py spec/examples/
python3 quasi-agent/cli.py --help > /dev/null
python3 quasi-agent/generate_issue.py --list-models
```

### quasi-board tests

```bash
pytest quasi-board/tests/ -v --tb=short
```

### afana tests

```bash
pytest afana/tests/ -v --tb=short
```

### Targeted smoke checks

Use targeted runs when you only changed one subsystem:

```bash
pytest quasi-board/tests/test_api.py -v
pytest afana/tests/test_cli.py -v
```

## Contribution workflow

1. Pick an issue and confirm scope before editing code.
2. Create a focused branch from `main`.
3. Keep changes scoped to the ticket.
4. Run the relevant test commands locally.
5. Open a pull request with a concise description and issue reference.

## Pull request checklist

Before submitting:

- verify the branch contains only ticket-related changes
- run the relevant tests for the touched component
- update docs when behavior or setup changed
- include the issue number in the PR body when applicable
