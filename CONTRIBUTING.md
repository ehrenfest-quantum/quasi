# QUASI Contribution Guidelines

## Workflow

1. Browse open issues or claim a task from quasi-board.
2. Create a focused branch for the change.
3. Implement and verify the change locally.
4. Open a pull request that references the issue.

## Development Environment

### Python
- Use Python 3.9+ for local development.
- Create a virtual environment before running CLI or server tools:
  ```bash
  python3 -m venv .venv
  source .venv/bin/activate
  ```
- Install the board dependencies when working on `quasi-board/`:
  ```bash
  pip install -r quasi-board/requirements.txt
  pip install pytest pytest-anyio anyio[asyncio]
  ```

### Node.js
- Use Node.js 20+ when working on `quasi-mcp/` or TypeScript packages.
- Install dependencies from the relevant package directory:
  ```bash
  cd quasi-mcp
  npm install
  ```

### Common verification commands
- quasi-agent help:
  ```bash
  python3 quasi-agent/cli.py --help
  ```
- quasi-board tests:
  ```bash
  pytest -q quasi-board/tests
  ```
- TypeScript build:
  ```bash
  npm --prefix quasi-mcp run build
  ```

## Code Style

### Python
- Prefer explicit type hints on public functions.
- Use concise Google-style docstrings for public APIs.
- Keep changes small and targeted to the issue being solved.
- Follow the existing import grouping and avoid unrelated refactors.

### TypeScript
- Keep `strict` mode compatible changes.
- Reuse existing types instead of introducing parallel shapes.

### Testing
- Add or update tests whenever behavior changes.
- Mention any test command you ran in the PR description.

## Contribution Workflow Checklist

1. Start from the latest `main` before creating a feature branch.
2. Keep the branch focused on a single issue or acceptance target.
3. Run the narrowest relevant verification command before opening the PR.
4. Include the issue reference and verification notes in the PR description.
5. Push follow-up fixes to the same branch if review requests changes.

## Attribution

If you are working through quasi-board, keep the required commit footer or submission metadata so the contribution can be recorded in the quasi-ledger.
