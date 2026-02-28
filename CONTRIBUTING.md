# Contributing to QUASI

## Getting started

1. Fork the repository.
2. Create a branch for one issue or task.
3. Make the smallest change that satisfies the acceptance criteria.
4. Open a PR with the issue number in the title or description.

## Development Environment

### Python
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r quasi-board/requirements.txt
pip install pytest pytest-anyio anyio[asyncio]
```

### Node.js
```bash
cd quasi-mcp
npm install
```

## Verification

Run the most relevant checks for the area you changed:

```bash
python3 quasi-agent/cli.py --help
pytest -q quasi-board/tests
npm --prefix quasi-mcp run build
```

## Workflow expectations

- Keep PRs tightly scoped.
- Reuse existing file structure and naming.
- Add or update tests when behavior changes.
- Mention the test command you ran in the PR description.

## Attribution and quasi-board

When working through quasi-board, preserve the required contribution metadata so the webhook or ledger fallback can record the contribution correctly.
