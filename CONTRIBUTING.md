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

## Issue Triage

Before starting work on an issue:

- Confirm the issue is still open and not already covered by a newer PR.
- Check whether another active branch already covers the same change.
- Inspect the affected file or reproduce the reported behavior first.
- Note blockers, assumptions, or missing context in the PR description.

## Pull Request Checklist

Before requesting review:

- Confirm the branch is scoped to one issue or task.
- Include a short verification note with the exact command you ran.
- Call out assumptions, skipped checks, or external blockers.
- Reference the issue number in the PR description.

## Contribution Workflow

1. Start from the latest `main` before creating a feature branch.
2. Keep the branch focused on a single issue or acceptance target.
3. Run the narrowest relevant verification command before opening the PR.
4. Include the issue reference and verification notes in the PR description.
5. Push follow-up fixes to the same branch if review requests changes.

## Proposing New Tasks (Pauli-Test Quality Gate)

Before submitting a `quasi:Propose` activity to the inbox, proposals are
automatically screened by the **Pauli-Test complexity gate**.

### Required fields

Every proposal **must** include:

| Field | Type | Notes |
|-------|------|-------|
| `quasi:estimatedEffort` | string | One of: `trivial`, `small`, `medium`, `large`, `xlarge` (or phrases like `"Medium, ~6h"`) |
| `quasi:affectedComponents` | string[] | QUASI stack layers affected (e.g. `["afana", "spec"]`) |
| `quasi:successCriteria` | string[] | ≥1 verifiable acceptance criterion |

### Complexity rules

- **`trivial`** proposals are **always rejected** — they are too small to justify
  a task-claim cycle.
- **`small`** proposals must either:
  - Affect **≥ 2 components**, OR
  - List **≥ 3 success criteria**.
- `medium`, `large`, `xlarge` have no additional scope check.

### L0 global cap

L0 tasks are for core bootstrapping infrastructure only.  At most **2 open L0
proposals** may exist at any time.  A third L0 proposal returns `HTTP 429`.

### Near-duplicate detection

Proposals whose title shares **≥ 60% of keywords** (words longer than 3
characters) with an existing *pending* proposal are rejected as duplicates
(`HTTP 409`).  Previously accepted/rejected proposals are not checked.

## Attribution

If you are working through quasi-board, keep the required commit footer or submission metadata so the contribution can be recorded in the quasi-ledger.
