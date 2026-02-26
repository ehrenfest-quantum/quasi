#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
solve.py — Autonomous issue solver for the QUASI Pauli-Test.

Picks an open GitHub issue, uses an eligible open-weights model to
generate a fix, commits it to a branch, opens a PR, and records the
completion on the quasi-ledger (Leaderboard B).

Usage:
    python3 quasi-agent/solve.py --issue 67 --model deepseek-v3
    python3 quasi-agent/solve.py --issue 67 --model deepseek-v3 --dry-run
    python3 quasi-agent/solve.py --list-open   # show open issues

The model must be in the ROTATION allowlist (same eligibility rules
as generate_issue.py). The PR will contain the commit footer:

    Contribution-Agent: <model-id>
    Closes: #<issue-number>
    Verification: ci-pass

Environment variables (same as generate_issue.py):
    GITHUB_TOKEN         required — read issues, create PRs
    OPENROUTER_API_KEY   for openrouter-hosted models
    HF_TOKEN             for HuggingFace Inference Router models
    SARVAM_API_KEY       for Sarvam models
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

_here = Path(__file__).parent
sys.path.insert(0, str(_here))

from generate_issue import ROTATION, PROVIDERS, find_rotation_entry  # noqa: E402

GITHUB_REPO = "ehrenfest-quantum/quasi"
GITHUB_API = f"https://api.github.com/repos/{GITHUB_REPO}"
BOARD_URL = "https://gawain.valiant-quantum.com"
REPO_DIR = _here.parent  # quasi repo root


# ── GitHub helpers ─────────────────────────────────────────────────────────────

def gh(method: str, path: str, body: dict | None = None) -> dict | list:
    token = os.environ.get("GITHUB_TOKEN", "")
    url = f"{GITHUB_API}{path}" if path.startswith("/") else path
    data = json.dumps(body).encode() if body else None
    req = urllib.request.Request(
        url, data=data, method=method,
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        body_text = e.read().decode()
        print(f"GitHub {method} {url} → {e.code}: {body_text[:300]}", file=sys.stderr)
        raise


def gh_get_all(path: str) -> list:
    results = []
    url: str | None = f"{GITHUB_API}{path}?per_page=100"
    while url:
        token = os.environ.get("GITHUB_TOKEN", "")
        req = urllib.request.Request(url, headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
        })
        with urllib.request.urlopen(req, timeout=30) as r:
            data = json.loads(r.read())
            results.extend(data)
            link = r.headers.get("Link", "")
            url = None
            for part in link.split(","):
                if 'rel="next"' in part:
                    m = re.search(r"<([^>]+)>", part)
                    if m:
                        url = m.group(1)
    return results


# ── Repo context builder ───────────────────────────────────────────────────────

# Files always included for context
CORE_CONTEXT_FILES = [
    "README.md",
    ".github/workflows/ci.yml",
    "quasi-board/server.py",
    "quasi-agent/cli.py",
    "quasi-agent/generate_issue.py",
]

# Label → extra files to include
LABEL_CONTEXT = {
    "specification": ["spec/ehrenfest-v0.1.cddl", "docs/BENCHMARK.md"],
    "compiler": ["spec/ehrenfest-v0.1.cddl"],
    "infrastructure": [".github/workflows/ci.yml"],
    "docs": ["README.md", "docs/ISSUE-GENERATION.md"],
    "infra": [".github/workflows/ci.yml"],
    "agent-ux": ["quasi-agent/cli.py", "quasi-agent/generate_issue.py"],
}

MAX_FILE_CHARS = 16000


def read_repo_file(rel_path: str) -> str | None:
    full = REPO_DIR / rel_path
    if not full.exists():
        return None
    try:
        text = full.read_text(encoding="utf-8", errors="replace")
        if len(text) > MAX_FILE_CHARS:
            text = text[:MAX_FILE_CHARS] + f"\n... (truncated at {MAX_FILE_CHARS} chars)"
        return text
    except Exception:
        return None


def build_context(issue: dict) -> str:
    labels = [line["name"] for line in issue.get("labels", [])]
    files_to_include = list(CORE_CONTEXT_FILES)
    for label in labels:
        files_to_include.extend(LABEL_CONTEXT.get(label, []))
    # deduplicate preserving order
    seen: set[str] = set()
    unique_files = []
    for f in files_to_include:
        if f not in seen:
            seen.add(f)
            unique_files.append(f)

    parts = [
        f"## Repository: {GITHUB_REPO}",
        f"## Issue #{issue['number']}: {issue['title']}",
        f"Labels: {', '.join(labels) or 'none'}",
        "",
        "### Issue body",
        issue.get("body", "(no body)"),
        "",
        "### Current repo files (for context)",
    ]
    for rel in unique_files:
        content = read_repo_file(rel)
        if content:
            parts.append(f"\n#### {rel}\n```\n{content}\n```")

    return "\n".join(parts)


# ── LLM call ──────────────────────────────────────────────────────────────────

SOLVER_SYSTEM = """You are an autonomous software agent solving a GitHub issue in the QUASI project.

You will receive:
1. The issue title, body, and acceptance criteria
2. Relevant current repo files for context

Your task: produce the MINIMAL edits needed to satisfy the acceptance criteria.

Respond with ONLY a JSON object (no markdown fences, no prose before or after):
{
  "reasoning": "one sentence explaining what you changed and why",
  "edits": [
    {
      "file": "path/to/file.md",
      "find": "exact string to search for (must exist verbatim in the file)",
      "replace": "replacement string"
    }
  ],
  "new_files": {
    "path/to/new/file.md": "complete content for NEW files that do not yet exist"
  }
}

Rules:
- Use "edits" for modifying EXISTING files. Each edit is a find/replace on the file.
  - "find" must be an exact verbatim substring of the current file content — not paraphrased.
  - "replace" is what it becomes. Can be empty string to delete.
  - Multiple edits on the same file are applied in order.
- Use "new_files" only for files that do not yet exist.
- If the issue is already fully satisfied, set "edits" to [] and explain in "reasoning".
- Do not modify lines unrelated to the issue.
- Keep changes minimal — only what satisfies the acceptance criteria.
"""


def call_model(entry: dict, prompt: str) -> dict:
    provider_id = entry["provider"]
    provider = PROVIDERS[provider_id]
    api_key = os.environ.get(provider["env"], "")
    if not api_key:
        print(f"Error: {provider['env']} not set", file=sys.stderr)
        sys.exit(1)

    # Some models cap max_tokens lower than 8192
    max_tok = entry.get("max_tokens", 8192)
    # Some models have small context windows — truncate if needed
    max_ctx = entry.get("max_context")
    if provider_id == "sarvam":
        prompt = prompt[:24000]
    elif max_ctx and len(prompt) > max_ctx:
        prompt = prompt[:max_ctx]
    payload = {
        "model": entry["model"],
        "messages": [
            {"role": "system", "content": SOLVER_SYSTEM},
            {"role": "user", "content": prompt},
        ],
        "max_tokens": max_tok,
        "temperature": 0.2,
    }

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        **provider.get("headers", {}),
    }

    req = urllib.request.Request(
        provider["url"],
        data=json.dumps(payload).encode(),
        headers=headers,
    )

    print(f"Calling {entry['model']} via {provider_id}...", end=" ", flush=True)
    try:
        timeout = 600 if provider_id == "huggingface" else 120
        with urllib.request.urlopen(req, timeout=timeout) as r:
            resp = json.loads(r.read())
    except urllib.error.HTTPError as e:
        print(f"HTTP {e.code}: {e.read().decode()[:200]}", file=sys.stderr)
        sys.exit(1)

    print("done")
    content = resp["choices"][0]["message"]["content"].strip()

    # Strip markdown fences if the model wrapped the JSON
    content = re.sub(r"^```(?:json)?\s*", "", content)
    content = re.sub(r"\s*```$", "", content)

    try:
        return json.loads(content)
    except json.JSONDecodeError:
        pass

    # Repair common model output issues before giving up
    repaired = content

    # 1. Replace Python triple-quoted strings with JSON-safe equivalent
    repaired = re.sub(
        r'\"\"\"(.*?)\"\"\"',
        lambda m: json.dumps(m.group(1)),
        repaired,
        flags=re.DOTALL,
    )

    # 2. Try ast.literal_eval for Python-style single-quote dicts
    if "'" in repaired:
        try:
            import ast as _ast
            parsed = _ast.literal_eval(repaired)
            if isinstance(parsed, dict):
                return parsed
        except Exception:
            pass

    # 3. Fix invalid JSON backslash escapes (e.g. \\( in regex patterns)
    # In JSON, only \\" \\\\ \\/ \\b \\f \\n \\r \\t \\uXXXX are valid
    import re as _re2
    repaired3 = _re2.sub(r"\\([^\"\\\\bfnrtu\/])", r"\\\\\\1", repaired)
    try:
        return json.loads(repaired3)
    except json.JSONDecodeError:
        pass

    try:
        return json.loads(repaired)
    except json.JSONDecodeError as e:
        print(f"Model output was not valid JSON: {e}", file=sys.stderr)
        print("Raw output:", file=sys.stderr)
        print(content[:1000], file=sys.stderr)
        sys.exit(1)


# ── Git + PR operations ────────────────────────────────────────────────────────

def apply_and_pr(
    issue: dict,
    model_entry: dict,
    result: dict,
    dry_run: bool = False,
) -> str | None:
    """
    Apply file changes, commit to a branch, push, open PR.
    Returns the PR URL, or None on dry-run.
    """
    import subprocess

    issue_num = issue["number"]
    model_id = model_entry["id"]
    branch = f"fix/issue-{issue_num}-{model_id}"
    edits = result.get("edits", [])
    new_files = result.get("new_files", {})
    reasoning = result.get("reasoning", "")

    if not edits and not new_files:
        print(f"Model says no changes needed: {reasoning}")
        return None

    changed_paths = list({e["file"] for e in edits}) + list(new_files.keys())
    print(f"\nFiles to change: {changed_paths}")
    print(f"Reasoning: {reasoning}")

    if dry_run:
        print("\n── Dry run — edits ──")
        for edit in edits:
            print(f"\n=== {edit['file']} ===")
            print(f"  FIND:    {edit['find'][:120]!r}")
            print(f"  REPLACE: {edit['replace'][:120]!r}")
        for path, content in new_files.items():
            print(f"\n=== NEW: {path} ===")
            print(content[:500])
            if len(content) > 500:
                print(f"... ({len(content)} total chars)")
        return None

    # Git operations
    def run(cmd: list[str]) -> str:
        r = subprocess.run(cmd, cwd=REPO_DIR, capture_output=True, text=True)
        if r.returncode != 0:
            print(f"git error: {r.stderr}", file=sys.stderr)
            raise RuntimeError(f"git command failed: {' '.join(cmd)}")
        return r.stdout.strip()

    # Ensure we're on main and up to date (stash any local dev changes)
    subprocess.run(["git", "stash"], cwd=REPO_DIR, capture_output=True)
    run(["git", "checkout", "main"])
    run(["git", "pull", "--rebase", "origin", "main"])
    # Create or reset branch
    try:
        run(["git", "checkout", "-b", branch])
    except RuntimeError:
        run(["git", "checkout", branch])
        run(["git", "reset", "--hard", "origin/main"])

    # Re-apply changes on the fresh branch
    for edit in edits:
        rel_path = edit["file"]
        find_str = edit["find"]
        replace_str = edit["replace"]
        full_path = REPO_DIR / rel_path
        if full_path.exists():
            original = full_path.read_text(encoding="utf-8")
            if find_str in original:
                full_path.write_text(original.replace(find_str, replace_str, 1), encoding="utf-8")
    for rel_path, content in new_files.items():
        full_path = REPO_DIR / rel_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_text(content, encoding="utf-8")

    for rel_path in changed_paths:
        run(["git", "add", rel_path])

    commit_msg = (
        f"fix: close #{issue_num} — {issue['title'][:60]}\n\n"
        f"{reasoning}\n\n"
        f"Contribution-Agent: {model_id}\n"
        f"Closes: #{issue_num}\n"
        f"Verification: ci-pass\n"
        f"Co-Authored-By: {model_entry['model']} <noreply@quasi.dev>"
    )
    run(["git", "commit", "-m", commit_msg])
    commit_hash = run(["git", "rev-parse", "HEAD"])

    run(["git", "push", "origin", branch, "--force"])
    print(f"Pushed branch: {branch} ({commit_hash[:12]})")

    # Open PR
    pr_body = (
        f"Closes #{issue_num}\n\n"
        f"**Solver:** `{model_id}` ({model_entry['model']})\n"
        f"**Provider:** {model_entry['provider']}\n"
        f"**License:** {model_entry['license']}\n"
        f"**Origin:** {model_entry['origin']}\n\n"
        f"**Reasoning:** {reasoning}\n\n"
        f"---\n"
        f"*Autonomous completion — Pauli-Test Leaderboard B*\n"
        f"*Contribution-Agent: {model_id}*"
    )
    pr_data = gh("POST", "/pulls", {
        "title": f"fix: #{issue_num} — {issue['title'][:60]}",
        "body": pr_body,
        "head": branch,
        "base": "main",
    })
    pr_url = pr_data["html_url"]
    print(f"PR opened: {pr_url}")

    # Record completion on quasi-ledger
    ledger_body = json.dumps({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "quasi:type": "completion",
        "actor": model_id,
        "quasi:taskId": f"GH-{issue_num}",
        "quasi:commitHash": commit_hash,
        "quasi:prUrl": pr_url,
        "published": datetime.now(timezone.utc).isoformat(),
    }).encode()
    ledger_req = urllib.request.Request(
        f"{BOARD_URL}/quasi-board/inbox",
        data=ledger_body,
        headers={"Content-Type": "application/json", "User-Agent": "quasi-agent/1.0"},
    )
    try:
        with urllib.request.urlopen(ledger_req, timeout=10) as r:
            ledger_resp = json.loads(r.read())
        print(f"Ledger entry: #{ledger_resp.get('ledger_entry')} "
              f"({ledger_resp.get('entry_hash','')[:16]}...)")
    except Exception as e:
        print(f"Ledger record failed (PR still open): {e}", file=sys.stderr)

    return pr_url


# ── CLI ────────────────────────────────────────────────────────────────────────

def list_open_issues() -> None:
    issues = gh_get_all("/issues?state=open")
    GENERATOR_PAT = re.compile(r"Generator model:\s*`([^`]+)`.*?Level:\s*L(\d+)",
                               re.IGNORECASE | re.DOTALL)
    ROSTER_MODELS = {e["model"] for e in ROTATION} | {e["id"] for e in ROTATION}
    print(f"\nOpen issues ({len(issues)} total):\n")
    for i in issues:
        if i.get("pull_request"):
            continue
        body = i.get("body", "")
        m = GENERATOR_PAT.search(body)
        gen = f"[{m.group(1)[:25]} L{m.group(2)}]" if m else "[human]"
        in_roster = m and m.group(1) in ROSTER_MODELS
        tag = "★" if in_roster else " "
        labels = ",".join(line["name"] for line in i.get("labels", []))
        print(f"  {tag}#{i['number']:3d} {gen:35s} [{labels}] {i['title'][:45]}")
    print("\n★ = generated by a roster model (Leaderboard B target)\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="QUASI autonomous issue solver — Pauli-Test Leaderboard B"
    )
    parser.add_argument("--issue", type=int, help="GitHub issue number to solve")
    parser.add_argument("--model", default="deepseek-v3",
                        help="Model short ID from ROTATION (default: deepseek-v3)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Show proposed changes without committing or opening PR")
    parser.add_argument("--list-open", action="store_true",
                        help="List open issues and exit")
    args = parser.parse_args()

    if args.list_open:
        list_open_issues()
        return

    if not args.issue:
        parser.error("--issue is required (or use --list-open)")

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("Error: GITHUB_TOKEN not set", file=sys.stderr)
        sys.exit(1)

    model_entry = find_rotation_entry(args.model)
    issue = gh("GET", f"/issues/{args.issue}")

    print("\n── QUASI Pauli-Test Solver ──")
    print(f"   Issue:    #{issue['number']} — {issue['title']}")
    print(f"   Model:    {model_entry['model']}")
    print(f"   Provider: {model_entry['provider']}  ({model_entry['origin']} · {model_entry['license']})")
    print(f"   Mode:     {'dry-run' if args.dry_run else 'LIVE'}")
    print()

    context = build_context(issue)
    print(f"Context built ({len(context)} chars)")

    result = call_model(model_entry, context)

    pr_url = apply_and_pr(issue, model_entry, result, dry_run=args.dry_run)

    if pr_url:
        print(f"\n✓ Solved by {model_entry['id']} — {pr_url}")
        print("  Leaderboard B entry pending PR merge + CI pass")
    elif args.dry_run:
        print("\nDry run complete.")
    else:
        print("\nNo changes needed — issue already satisfied.")


if __name__ == "__main__":
    main()
