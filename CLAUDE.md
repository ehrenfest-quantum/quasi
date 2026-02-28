# QUASI — Claude Code Instructions

## Project

QUASI is a quantum OS project. The primary repo is at `ehrenfest-quantum/quasi` on GitHub.
The main components are:
- `afana/` — Ehrenfest compiler (Python)
- `quasi-board/` — ActivityPub task ledger server (FastAPI)
- `spec/` — Ehrenfest language spec (CDDL, README)
- `examples/` — Sample `.ef` programs (bell, ghz, grover, teleport)

## Issue Priority

**Always work Ehrenfest and Afana issues first.** In order:
1. `compiler` + `core` labelled issues
2. `compiler` labelled issues
3. `specification` labelled issues
4. Other open issues

Skip issues that are already implemented — check the codebase before starting.
Close already-done issues with an explanatory comment rather than re-implementing.

## Autonomous Work Loop

When working autonomously (overnight / unattended), follow this loop:

1. `gh issue list --repo ehrenfest-quantum/quasi --state open --json number,title,labels --limit 40`
2. Pick the highest-priority Ehrenfest/Afana issue not yet started
3. Read the issue thoroughly
4. Check if it is already implemented — if so, close it with a comment and pick the next one
5. Checkout a new branch: `git checkout -b feat/<short-name>`
6. Implement the feature or fix — read existing code first, match conventions
7. Write tests — always add tests alongside implementation
8. Run tests locally before committing
9. Commit with a descriptive message and `Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>`
10. Push and open a PR with `gh pr create`
11. Pull `main` and repeat from step 1

## Hard Rules

- **Never commit directly to main** — always branch + PR
- **Never push --force**
- **Never skip tests** — if tests fail, fix them before committing
- **Always run flake8** on changed Python files before committing (max-line-length=120)
- Pull `main` fresh before starting each new issue
- Keep PRs focused — one issue per PR
- Do not open a PR if CI is red — fix lint/test failures first

## Server Access

- **Camelot**: `root@87.106.219.154` — quasi-board production server
- **Voila** (`46.224.51.129`) is a completely separate company — NEVER access it for quasi work
- SSH aliases: use `camelot` if configured, otherwise use the IP directly

## Python

- Use Python 3.10+ syntax (`X | Y` union types, match statements)
- No external dependencies beyond what is already in `requirements.txt` unless the issue explicitly requires one
- Always add new public symbols to `__all__` and module `__init__.py`
