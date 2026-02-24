#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
generate_issue.py — Stage 1 of the Pauli-Test issue generation protocol.

A model from the eligible rotation analyses the current QUASI project state
and writes one GitHub issue. A different model (or human) will solve it.

Usage:
    python3 quasi-agent/generate_issue.py --level 0 --dry-run
    python3 quasi-agent/generate_issue.py --level 0 --model deepseek/deepseek-chat-v3-0324

Environment:
    OPENROUTER_API_KEY   Required. Get one at openrouter.ai.
    QUASI_GENERATOR_MODEL  Optional. Overrides --model flag.
    GITHUB_TOKEN         Optional. Required to open a draft GitHub issue.

See docs/ISSUE-GENERATION.md for the full protocol.
"""

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

# ── Constants ────────────────────────────────────────────────────────────────

OPENROUTER_API = "https://openrouter.ai/api/v1/chat/completions"
GITHUB_API = "https://api.github.com/repos/ehrenfest-quantum/quasi/issues"
BOARD_LEDGER = "https://gawain.valiant-quantum.com/quasi-board/ledger"
BOARD_INBOX = "https://gawain.valiant-quantum.com/quasi-board/inbox"
REPO_URL = "https://github.com/ehrenfest-quantum/quasi"

# Rotation pool — models eligible for Stage 1 (issue generation).
# No model may write two consecutive issues at the same level.
# See docs/ELIGIBLE-MODELS.md for the full roster.
ROTATION = [
    "deepseek/deepseek-chat-v3-0324",       # DeepSeek-V3 (MIT)
    "deepseek/deepseek-r1",                  # DeepSeek-R1 (MIT)
    "qwen/qwen-2.5-coder-32b-instruct",      # Qwen2.5-Coder (Qwen license)
    "meta-llama/llama-4-maverick",           # Llama 4 Maverick
    "meta-llama/llama-3.3-70b-instruct",     # Llama 3.3 70B
    "mistralai/mistral-small-3.1-24b-instruct",  # Mistral Small 3.1 (Apache 2.0)
    "mistralai/mistral-nemo",                # Mistral Nemo (Apache 2.0)
    "ai21/jamba-1-5-mini",                   # Jamba 1.5 Mini (Apache 2.0)
    "tiiuae/falcon3-10b-instruct",           # Falcon 3 10B (Apache 2.0)
]

DEFAULT_MODEL = "deepseek/deepseek-chat-v3-0324"

LEVEL_NAMES = {
    0: "L0 — Scaffolding (README, badges, CI config, docs)",
    1: "L1 — Language Foundations (Ehrenfest syntax, parser, AST, type system)",
    2: "L2 — Compiler / Afana (ZX-IR generation, rewriting rules, QASM3 output)",
    3: "L3 — Hardware Backends (IBM/IQM adapters, HAL Contract, error mitigation)",
    4: "L4 — Turing-Complete Runtime (quantum memory model, classical control flow)",
}

LABEL_TAXONOMY = "compiler · specification · infrastructure · agent-ux · good-first-issue"

# ── Repo context ──────────────────────────────────────────────────────────────

def repo_root() -> Path:
    """Find the repo root from this script's location."""
    here = Path(__file__).resolve().parent
    # script is in quasi-agent/, repo root is one level up
    return here.parent


def file_tree(root: Path, max_files: int = 60) -> str:
    """Compact file tree — enough for a model to understand project structure."""
    SKIP = {".git", "__pycache__", "node_modules", ".venv", "dist", "build",
            "target", ".next", "htmlcov", "coverage"}
    lines = []
    count = 0
    for p in sorted(root.rglob("*")):
        if count >= max_files:
            lines.append("  ... (truncated)")
            break
        parts = set(p.relative_to(root).parts)
        if parts & SKIP or any(x.startswith(".") for x in p.relative_to(root).parts):
            continue
        if p.is_file():
            rel = str(p.relative_to(root))
            lines.append(f"  {rel}")
            count += 1
    return "\n".join(lines)


def recent_commits(root: Path, n: int = 10) -> str:
    """Last n commit subjects via git log."""
    import subprocess
    try:
        result = subprocess.run(
            ["git", "log", f"--max-count={n}", "--oneline"],
            cwd=root, capture_output=True, text=True, timeout=5,
        )
        return result.stdout.strip() or "(no commits)"
    except Exception:
        return "(git unavailable)"


def open_issues_summary() -> str:
    """Fetch open tasks from the quasi-board."""
    req = urllib.request.Request(
        "https://gawain.valiant-quantum.com/quasi-board/outbox",
        headers={"Accept": "application/json", "User-Agent": "quasi-agent/generate_issue"},
    )
    try:
        with urllib.request.urlopen(req, timeout=8) as resp:
            data = json.loads(resp.read())
        items = data.get("orderedItems", [])
        if not items:
            return "No open tasks on the quasi-board."
        lines = []
        for item in items[:10]:
            t = item.get("object", item) if item.get("type") == "Create" else item
            task_id = t.get("quasi:taskId", "?")
            title = t.get("name", "(no title)")
            status = t.get("quasi:status", "open")
            lines.append(f"  {task_id}: {title} [{status}]")
        return "\n".join(lines)
    except Exception as e:
        return f"(could not reach quasi-board: {e})"


def build_context(level: int, root: Path) -> str:
    """Assemble the full context passed to the generator model."""
    tree = file_tree(root)
    commits = recent_commits(root)
    issues = open_issues_summary()
    level_name = LEVEL_NAMES.get(level, f"L{level}")

    return f"""You are analysing the QUASI Quantum OS project.
Repository: {REPO_URL}

## Project overview

QUASI is an open-source quantum operating system. Key components:
- **Ehrenfest** — a quantum programming language (AI-primary, CBOR binary format)
- **Afana** — compiler (ZX-IR → QASM3), named after Tatiana Afanasyeva
- **quasi-board** — ActivityPub task board with a SHA256 hash-linked ledger
- **quasi-agent** — CLI for task management and ledger interaction

The project is at an early stage. The Pauli-Test benchmark measures AI engineering
agents by having them resolve GitHub issues from this project.

## Current file tree

{tree}

## Recent commits

{commits}

## Open tasks on the quasi-board

{issues}

## Capability Ladder

{chr(10).join(f'- {v}' for v in LEVEL_NAMES.values())}

Current frontier level: **{level_name}**

## Label taxonomy

{LABEL_TAXONOMY}

## Your task

Identify what the project needs next at the current frontier level ({level_name}).

Write one GitHub issue. Requirements:
- Title: concise, imperative, specific (not "improve X" — say exactly what to do)
- Description: ≥3 sentences explaining context, why this matters, what approach to take
- Acceptance criteria: ≥2 bullet points, each CI-verifiable (a passing test, a file that exists, a command that succeeds)
- Label: exactly one from the taxonomy above

Output ONLY valid JSON in this exact structure — no prose before or after:

{{
  "title": "...",
  "description": "...",
  "acceptance_criteria": ["...", "..."],
  "label": "..."
}}"""


# ── OpenRouter call ───────────────────────────────────────────────────────────

def call_openrouter(model: str, prompt: str, api_key: str) -> str:
    """Call the OpenRouter chat completions endpoint. Returns the response text."""
    body = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 1024,
        "temperature": 0.7,
    }).encode()

    req = urllib.request.Request(
        OPENROUTER_API,
        data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://quasi.arvak.io",
            "X-Title": "QUASI Pauli-Test issue generator",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read())
    except urllib.error.HTTPError as e:
        body_text = e.read().decode()
        print(f"OpenRouter error {e.code}: {body_text}", file=sys.stderr)
        sys.exit(1)

    return data["choices"][0]["message"]["content"].strip()


def parse_issue(raw: str) -> dict:
    """Extract JSON from model output — handles markdown code fences."""
    # Strip ```json ... ``` if present
    text = raw
    if "```" in text:
        lines = text.split("\n")
        in_block = False
        cleaned = []
        for line in lines:
            if line.startswith("```"):
                in_block = not in_block
                continue
            if in_block or not text.count("```"):
                cleaned.append(line)
        text = "\n".join(cleaned)
    # Find the first { ... } block
    start = text.find("{")
    end = text.rfind("}") + 1
    if start == -1 or end == 0:
        raise ValueError(f"No JSON object found in model output:\n{raw}")
    return json.loads(text[start:end])


# ── GitHub draft issue ────────────────────────────────────────────────────────

def open_github_issue(issue: dict, model: str, level: int, github_token: str) -> str:
    """Open a draft GitHub issue. Returns the issue URL."""
    criteria_md = "\n".join(f"- [ ] {c}" for c in issue["acceptance_criteria"])
    body = f"""{issue['description']}

## Acceptance criteria

{criteria_md}

---
*Generated by the Pauli-Test issue generation protocol.*
*Generator model: `{model}` · Level: L{level} · Date: {datetime.now(timezone.utc).date()}*
*See [docs/ISSUE-GENERATION.md](https://github.com/ehrenfest-quantum/quasi/blob/main/docs/ISSUE-GENERATION.md) for protocol details.*"""

    payload = json.dumps({
        "title": issue["title"],
        "body": body,
        "labels": [issue["label"]],
    }).encode()

    req = urllib.request.Request(
        GITHUB_API,
        data=payload,
        headers={
            "Authorization": f"Bearer {github_token}",
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "User-Agent": "quasi-agent/generate_issue",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())
            return data.get("html_url", "?")
    except urllib.error.HTTPError as e:
        print(f"GitHub error {e.code}: {e.read().decode()}", file=sys.stderr)
        sys.exit(1)


# ── Ledger entry ──────────────────────────────────────────────────────────────

def record_ledger(model: str, level: int, issue_url: str) -> None:
    """Append a generation event to the quasi-ledger."""
    body = json.dumps({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "quasi:type": "issue_generated",
        "quasi:level": level,
        "quasi:generator_model": model,
        "quasi:generator_provider": "openrouter",
        "quasi:issueUrl": issue_url,
        "published": datetime.now(timezone.utc).isoformat(),
    }).encode()

    req = urllib.request.Request(
        BOARD_INBOX,
        data=body,
        headers={"Content-Type": "application/json", "User-Agent": "quasi-agent/generate_issue"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read())
            print(f"Ledger entry: #{data.get('ledger_entry')} — {data.get('entry_hash', '')[:16]}...")
    except Exception as e:
        print(f"Warning: could not record ledger entry: {e}", file=sys.stderr)


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="QUASI Pauli-Test — Stage 1 issue generator",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Examples:
  # Dry run — print draft without opening a GitHub issue
  python3 quasi-agent/generate_issue.py --level 0 --dry-run

  # Use a specific model
  python3 quasi-agent/generate_issue.py --level 0 --model mistralai/mistral-nemo --dry-run

  # Open a real GitHub draft issue (requires GITHUB_TOKEN)
  python3 quasi-agent/generate_issue.py --level 0
""",
    )
    parser.add_argument("--level", type=int, default=0, choices=range(5),
                        help="Capability Ladder level to target (default: 0)")
    parser.add_argument("--model", default=None,
                        help=f"OpenRouter model ID (default: {DEFAULT_MODEL})")
    parser.add_argument("--dry-run", action="store_true",
                        help="Print the draft issue without opening it on GitHub")
    parser.add_argument("--list-models", action="store_true",
                        help="Print the eligible model rotation and exit")
    args = parser.parse_args()

    if args.list_models:
        print("\nEligible generation models (OpenRouter IDs):\n")
        for m in ROTATION:
            print(f"  {m}")
        print()
        return

    # Resolve model
    model = (
        args.model
        or os.environ.get("QUASI_GENERATOR_MODEL")
        or DEFAULT_MODEL
    )

    # Resolve API key
    api_key = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if not api_key:
        print("Error: OPENROUTER_API_KEY not set.", file=sys.stderr)
        print("  export OPENROUTER_API_KEY=sk-or-...", file=sys.stderr)
        sys.exit(1)

    root = repo_root()
    level = args.level

    print(f"\n── QUASI Pauli-Test Issue Generator ──")
    print(f"   Model:  {model}")
    print(f"   Level:  L{level} — {LEVEL_NAMES[level]}")
    print(f"   Repo:   {root}")
    print(f"   Mode:   {'dry-run (no GitHub issue)' if args.dry_run else 'live'}")
    print()

    print("Building context...", end=" ", flush=True)
    prompt = build_context(level, root)
    print(f"done ({len(prompt)} chars)")

    print(f"Calling {model}...", end=" ", flush=True)
    raw = call_openrouter(model, prompt, api_key)
    print("done")

    print("\n── Raw model output ──")
    print(raw)
    print()

    try:
        issue = parse_issue(raw)
    except (ValueError, json.JSONDecodeError) as e:
        print(f"Error parsing model output: {e}", file=sys.stderr)
        print("Raw output saved above. Check that the model returned valid JSON.", file=sys.stderr)
        sys.exit(1)

    print("── Parsed issue ──")
    print(f"Title:  {issue.get('title', '?')}")
    print(f"Label:  {issue.get('label', '?')}")
    print(f"Desc:   {issue.get('description', '?')[:120]}...")
    print("Acceptance criteria:")
    for c in issue.get("acceptance_criteria", []):
        print(f"  - {c}")
    print()

    # Validate minimum requirements
    missing = []
    if not issue.get("title"):
        missing.append("title")
    if not issue.get("description") or len(issue["description"]) < 50:
        missing.append("description (too short)")
    if len(issue.get("acceptance_criteria", [])) < 2:
        missing.append("acceptance_criteria (need ≥2)")
    if issue.get("label") not in LABEL_TAXONOMY:
        print(f"Warning: label '{issue.get('label')}' not in taxonomy — check before publishing")
    if missing:
        print(f"Warning: issue is missing: {', '.join(missing)}", file=sys.stderr)

    if args.dry_run:
        print("Dry run complete — issue not opened on GitHub.")
        return

    github_token = os.environ.get("GITHUB_TOKEN", "").strip()
    if not github_token:
        print("Error: GITHUB_TOKEN not set. Required to open a GitHub issue.", file=sys.stderr)
        print("  Re-run with --dry-run to skip GitHub, or set GITHUB_TOKEN.", file=sys.stderr)
        sys.exit(1)

    print("Opening GitHub issue...", end=" ", flush=True)
    issue_url = open_github_issue(issue, model, level, github_token)
    print(f"done\n  {issue_url}")

    print("Recording ledger entry...", end=" ", flush=True)
    record_ledger(model, level, issue_url)

    print(f"\n✓ Issue generated at L{level} by {model}")
    print(f"  {issue_url}")
    print()


if __name__ == "__main__":
    main()
