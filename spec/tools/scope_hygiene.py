#!/usr/bin/env python3
"""
Quality Radar — Dimension ① Scope Hygiene
==========================================
Checks that the files changed in a PR are relevant to the issue label.
Fails if fewer than 50% of non-trivial changed files match the label's
expected path patterns.

Usage (in CI):
  python3 spec/tools/scope_hygiene.py

Environment variables required:
  GITHUB_TOKEN          — for GitHub API calls (auto-injected by Actions)
  GITHUB_REPOSITORY     — e.g. ehrenfest-quantum/quasi (auto-injected)
  GITHUB_BASE_REF       — target branch (auto-injected on pull_request)
  PR_BODY               — PR body text (pass via ${{ github.event.pull_request.body }})
  PR_TITLE              — PR title (pass via ${{ github.event.pull_request.title }})

Exit codes:
  0 — pass (scope hygiene met, or check not applicable)
  1 — fail (scope violation: < 50% relevant files)
"""

import os
import re
import sys
import json
import subprocess
import urllib.request
import urllib.error


# ── Label → expected path patterns ────────────────────────────────────────────
# Patterns are matched against file paths using re.search().
# An empty list means "any path is acceptable" (permissive label).
LABEL_PATTERNS: dict[str, list[str]] = {
    "compiler":        [r"^afana/", r"^spec/", r"^quasi-agent/solve\.py"],
    "specification":   [r"^spec/", r"^examples/"],
    "infrastructure":  [r"^quasi-board/", r"^\.github/workflows/",
                        r"^docker-compose", r"^Dockerfile"],
    "agent-ux":        [r"^quasi-agent/", r"^quasi-mcp/"],
    "docs":            [r"\.md$", r"^docs/", r"^examples/", r"\.py$"],
}

# Labels that are permissive — scope check skipped entirely
PERMISSIVE_LABELS = {"good-first-issue", "meta", "question", "wontfix"}

# Paths always considered in-scope regardless of label
ALWAYS_IN_SCOPE: list[str] = [
    r"^\.github/CODEOWNERS$",
    r"^CONTRIBUTING\.md$",
    r"^README\.md$",
    r"^\.gitignore$",
    r"^GENESIS\.md$",
    r"^ROADMAP\.md$",
]

THRESHOLD = 0.50  # minimum fraction of in-scope files required


# ── Helpers ───────────────────────────────────────────────────────────────────

def gh_api(path: str) -> dict | list | None:
    token = os.environ.get("GITHUB_TOKEN", "")
    repo  = os.environ.get("GITHUB_REPOSITORY", "ehrenfest-quantum/quasi")
    url   = f"https://api.github.com/repos/{repo}/{path}"
    req   = urllib.request.Request(url, headers={
        "Authorization": f"Bearer {token}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    })
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"  ⚠  GitHub API {url}: HTTP {e.code}", file=sys.stderr)
        return None
    except Exception as e:
        print(f"  ⚠  GitHub API error: {e}", file=sys.stderr)
        return None


def changed_files() -> list[str]:
    """Return list of files changed vs. the base branch."""
    base = os.environ.get("GITHUB_BASE_REF", "main")
    try:
        r = subprocess.run(
            ["git", "diff", "--name-only", f"origin/{base}...HEAD"],
            capture_output=True, text=True, check=True,
        )
        return [f.strip() for f in r.stdout.splitlines() if f.strip()]
    except subprocess.CalledProcessError as e:
        print(f"  ⚠  git diff failed: {e}", file=sys.stderr)
        return []


def extract_issue_number(text: str) -> int | None:
    """Find the first issue reference in a PR title or body."""
    # Matches: Closes #123  /  fix: #123  /  GH-123  /  #123
    for pat in [
        r"[Cc]loses?\s+#(\d+)",
        r"[Ff]ix(?:es|ed)?\s*:?\s*#(\d+)",
        r"\bGH-(\d+)\b",
        r"#(\d+)",
    ]:
        m = re.search(pat, text or "")
        if m:
            return int(m.group(1))
    return None


def issue_labels(issue_num: int) -> list[str]:
    data = gh_api(f"issues/{issue_num}")
    if not isinstance(data, dict):
        return []
    return [lbl["name"].lower() for lbl in data.get("labels", [])]


def matches_any(path: str, patterns: list[str]) -> bool:
    return any(re.search(p, path) for p in patterns)


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> int:
    pr_title = os.environ.get("PR_TITLE", "")
    pr_body  = os.environ.get("PR_BODY",  "")

    print("── Scope Hygiene Check (Quality Radar ①) ──────────────────────────────")

    # 1. Get changed files
    files = changed_files()
    if not files:
        print("  ℹ  No changed files detected — skipping.")
        return 0
    print(f"  Changed files ({len(files)}): {', '.join(files[:8])}{'…' if len(files) > 8 else ''}")

    # 2. Extract issue number
    issue_num = extract_issue_number(pr_title) or extract_issue_number(pr_body)
    if issue_num is None:
        print("  ℹ  No issue reference found in PR title/body — scope check skipped.")
        return 0
    print(f"  Referenced issue: #{issue_num}")

    # 3. Fetch issue labels
    labels = issue_labels(issue_num)
    print(f"  Issue labels: {labels or '(none)'}")

    # 4. If any label is permissive → skip
    if not labels or any(lbl in PERMISSIVE_LABELS for lbl in labels):
        print("  ℹ  Permissive or unlabeled issue — scope check skipped.")
        return 0

    # 5. Build expected patterns from labels
    expected: list[str] = []
    strict_labels: list[str] = []
    for lbl in labels:
        if lbl in LABEL_PATTERNS:
            expected.extend(LABEL_PATTERNS[lbl])
            strict_labels.append(lbl)

    if not expected:
        print(f"  ℹ  Labels {labels} have no registered patterns — scope check skipped.")
        return 0

    print(f"  Strict labels matched: {strict_labels}")
    print(f"  Expected path patterns: {expected}")

    # 6. Score each file
    in_scope     = []
    out_of_scope = []

    for f in files:
        if matches_any(f, ALWAYS_IN_SCOPE):
            in_scope.append(f"  ✓ {f}  (always in-scope)")
        elif matches_any(f, expected):
            in_scope.append(f"  ✓ {f}")
        else:
            out_of_scope.append(f"  ✗ {f}")

    total = len(files)
    score = len(in_scope) / total if total else 1.0

    for line in in_scope:
        print(line)
    for line in out_of_scope:
        print(line)

    print(f"\n  Scope hygiene: {len(in_scope)}/{total} files in-scope"
          f"  ({score:.0%} — threshold {THRESHOLD:.0%})")

    if score < THRESHOLD:
        print(f"\n❌ SCOPE VIOLATION: only {score:.0%} of changed files are relevant"
              f" to issue #{issue_num} (labels: {strict_labels}).")
        print(f"   At least {THRESHOLD:.0%} of changed files must relate to"
              f" the issue labels.\n")
        return 1

    print("\n✅ Scope hygiene passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
