#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
generate_issue.py — Stage 1 of the Pauli-Test issue generation protocol.

A model from the eligible rotation analyses the current QUASI project state
and writes one GitHub issue. A different model (or human) will solve it.

Usage:
    python3 quasi-agent/generate_issue.py --level 0 --dry-run
    python3 quasi-agent/generate_issue.py --level 0 --model sarvam-m
    python3 quasi-agent/generate_issue.py --level 0 --model deepseek/deepseek-chat-v3-0324

Environment variables (set only the keys for providers you want to use):
    OPENROUTER_API_KEY   openrouter.ai — covers most models
    SARVAM_API_KEY       api.sarvam.ai — Sarvam-30B/105B (India)
    MISTRAL_API_KEY      api.mistral.ai — direct Mistral endpoint
    HF_TOKEN             HuggingFace Inference Router (router.huggingface.co)
    CSCS_SERVING_API     CSCS Swiss AI API (api.swissai.cscs.ch) — Apertus-70B
                         Register at https://serving.swissai.cscs.ch
    QUASI_GENERATOR_MODEL  Optional model override (short ID or full model string)
    GITHUB_TOKEN         Required to open a real GitHub issue (not needed for --dry-run)

See docs/ISSUE-GENERATION.md for the full protocol.
See docs/ELIGIBLE-MODELS.md for the model roster and licensing.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

# ── Constants ────────────────────────────────────────────────────────────────

GITHUB_API = "https://api.github.com/repos/ehrenfest-quantum/quasi/issues"
BOARD_INBOX = "https://gawain.valiant-quantum.com/quasi-board/inbox"
REPO_URL = "https://github.com/ehrenfest-quantum/quasi"

# ── Providers ─────────────────────────────────────────────────────────────────
#
# Each provider is an OpenAI-compatible /v1/chat/completions endpoint.
# Add new direct-API providers here — no other code changes needed.
#
# Fields:
#   url      — chat completions endpoint
#   env      — environment variable holding the API key
#   headers  — extra headers beyond Authorization and Content-Type
#   verified — True if we can cross-check the served model name in the response
#              (prevents a provider from silently routing to a different model)

PROVIDERS: dict[str, dict] = {
    "openrouter": {
        "url": "https://openrouter.ai/api/v1/chat/completions",
        "env": "OPENROUTER_API_KEY",
        "headers": {
            "HTTP-Referer": "https://quasi.arvak.io",
            "X-Title": "QUASI Pauli-Test issue generator",
        },
        # OpenRouter returns X-Finalized-Model header with the actual model served
        "verify_header": "x-finalized-model",
    },
    "sarvam": {
        "url": "https://api.sarvam.ai/v1/chat/completions",
        "env": "SARVAM_API_KEY",
        "headers": {},
        "verify_header": None,
    },
    "mistral": {
        "url": "https://api.mistral.ai/v1/chat/completions",
        "env": "MISTRAL_API_KEY",
        "headers": {},
        "verify_header": None,
    },
    "huggingface": {
        "url": "https://router.huggingface.co/v1/chat/completions",
        "env": "HF_TOKEN",
        # User-Agent required — HF router proxies through Cloudflare-protected
        # backends that block Python's default urllib user agent.
        "headers": {"User-Agent": "quasi-agent/1.0 (https://quasi.arvak.io)"},
        "verify_header": None,
    },
    # Swiss National Supercomputing Centre (CSCS) — hosts Apertus-70B
    # Register at https://serving.swissai.cscs.ch to obtain a token.
    # Env var name matches their official docs and CLI repo.
    # Primary: api.research.computer (sk-rc-* tokens issued by serving.swissai.cscs.ch)
    # Fallback: api.swissai.cscs.ch (different token space, ignore if primary works)
    "swissai": {
        "url": "https://api.research.computer/v1/chat/completions",
        "env": "CSCS_SERVING_API",
        "headers": {},
        "verify_header": None,
    },
}

# ── Eligible model rotation ───────────────────────────────────────────────────
#
# This is the ALLOWLIST. Only models listed here may be used for issue
# generation. The script refuses any model not in this list.
#
# ANTI-MASKING RULE: models must be served by a verified open-weights provider.
# A closed-weights model (Claude, GPT, Gemini, Grok) cannot be in this list
# regardless of what a provider claims. The provider field must resolve to a
# real entry in PROVIDERS above. OpenRouter routes to the stated model; we
# cross-check the X-Finalized-Model response header where available.
#
# Format: {"id": short name for --model flag,
#           "model": API model string,
#           "provider": key in PROVIDERS,
#           "license": SPDX or brief license name,
#           "origin": country/org for coverage tracking}

ROTATION: list[dict] = [
    # ── Tier 1 — Strong coding ────────────────────────────────────────────
    {"id": "deepseek-v3", "model": "deepseek/deepseek-chat-v3-0324",
     "provider": "openrouter", "license": "MIT", "origin": "China / DeepSeek"},
    {"id": "deepseek-r1", "model": "deepseek/deepseek-r1",
     "provider": "openrouter", "license": "MIT", "origin": "China / DeepSeek"},
    {"id": "qwen3-coder", "model": "qwen/qwen3-coder",
     "provider": "openrouter", "license": "Apache-2.0", "origin": "China / Alibaba"},
    {"id": "llama4", "model": "meta-llama/llama-4-maverick",
     "provider": "openrouter", "license": "Llama Community", "origin": "US / Meta"},
    {"id": "llama3.3", "model": "meta-llama/llama-3.3-70b-instruct",
     "provider": "openrouter", "license": "Llama Community", "origin": "US / Meta"},
    # ── Tier 2 — EU / competitive coding ─────────────────────────────────
    {"id": "mistral-small", "model": "mistralai/mistral-small-3.1-24b-instruct",
     "provider": "openrouter", "license": "Apache-2.0", "origin": "France / Mistral"},
    {"id": "mistral-nemo", "model": "mistralai/mistral-nemo",
     "provider": "openrouter", "license": "Apache-2.0", "origin": "France / Mistral"},
    # ── Tier 1 (HF router) — strong coding, newly accessible ─────────────
    {"id": "kimi-k2", "model": "moonshotai/Kimi-K2-Instruct",
     "provider": "huggingface", "license": "Modified MIT",
     "origin": "China / Moonshot AI"},
    {"id": "glm-4.7", "model": "zai-org/GLM-4.7",
     "provider": "huggingface", "license": "MIT",
     "origin": "China / Zhipu AI"},
    # ── Tier 2 — EU / competitive coding (HF router) ─────────────────────
    {"id": "eurollm-22b", "model": "utter-project/EuroLLM-22B-Instruct-2512",
     "provider": "huggingface", "license": "Apache-2.0",
     "origin": "EU consortium / Unbabel (Portugal)"},
    {"id": "olmo-32b", "model": "allenai/olmo-3.1-32b-instruct",
     "provider": "openrouter", "license": "Apache-2.0",
     "origin": "US / Allen AI (fully open)"},
    # ── Tier 3 — Regional participation ──────────────────────────────────
    {"id": "sarvam-m", "model": "sarvam-m", "max_tokens": 1500,
     "provider": "sarvam", "license": "Open", "origin": "India / Sarvam AI"},
    {"id": "jamba", "model": "ai21/jamba-large-1.7",
     "provider": "openrouter", "license": "Jamba Open", "origin": "Israel / AI21"},
    {"id": "dicta", "model": "dicta-il/DictaLM-3.0-24B-Thinking",
     "provider": "huggingface", "license": "Apache-2.0",
     "origin": "Israel / Dicta (Bar-Ilan University)"},
    {"id": "swallow-70b", "model": "tokyotech-llm/Llama-3.3-Swallow-70B-Instruct-v0.4",
     "provider": "huggingface", "license": "Llama 3.3 Community",
     "origin": "Japan / Tokyo Institute of Technology"},
    {"id": "sea-lion", "model": "aisingapore/Qwen-SEA-LION-v4-32B-IT",
     "provider": "huggingface", "license": "Apache-2.0",
     "origin": "Singapore / AI Singapore"},
    {"id": "ernie-4.5", "model": "baidu/ernie-4.5-21b-a3b",
     "provider": "openrouter", "license": "ERNIE Open",
     "origin": "China / Baidu"},
    # falcon: no OpenRouter ID as of 2026-02-24 — use AI71 platform or self-host
    # {"id": "falcon", "model": "tiiuae/falcon3-10b-instruct",
    #  "provider": "openrouter", "license": "Apache-2.0", "origin": "UAE / TII"},
    {"id": "apertus", "model": "swiss-ai/Apertus-70B-Instruct-2509",
     "provider": "huggingface", "license": "Fully open",
     "origin": "Switzerland / ETH Zurich + EPFL + CSCS"},
    # ── Tier 1 additions — strong reasoning / coding (HF router) ─────────
    {"id": "qwq-32b", "model": "qwen/qwq-32b",
     "provider": "openrouter", "license": "Apache-2.0",
     "origin": "China / Alibaba (Qwen — reasoning model)"},
    {"id": "qwen3-30b", "model": "qwen/qwen3-30b-a3b-instruct-2507",
     "provider": "openrouter", "license": "Apache-2.0",
     "origin": "China / Alibaba (Qwen3 MoE, 30B total / 3B active)"},
    {"id": "gemma-3-27b", "model": "google/gemma-3-27b-it",
     "provider": "huggingface", "license": "Gemma",
     "origin": "US / Google DeepMind"},
    {"id": "command-a", "model": "cohere/command-a",
     "provider": "openrouter", "license": "CC-BY-NC-4.0",
     "origin": "Canada / Cohere (Toronto)"},
    # ── OpenRouter additions — confirmed working Feb 2026 ─────────────────
    {"id": "phi-4", "model": "microsoft/phi-4",
     "provider": "openrouter", "license": "MIT",
     "origin": "US / Microsoft Research", "max_context": 24000},
    {"id": "nemotron-70b", "model": "nvidia/llama-3.1-nemotron-70b-instruct",
     "provider": "openrouter", "license": "NVIDIA Open Model",
     "origin": "US / NVIDIA Research"},
    {"id": "hermes-3", "model": "nousresearch/hermes-3-llama-3.1-70b",
     "provider": "openrouter", "license": "Llama Community",
     "origin": "US / Nous Research"},
    {"id": "qwen2.5-72b", "model": "qwen/qwen-2.5-72b-instruct",
     "provider": "openrouter", "license": "Qwen Community",
     "origin": "China / Alibaba (general, not code-specific)"},
    {"id": "gemma-3-12b", "model": "google/gemma-3-12b-it",
     "provider": "openrouter", "license": "Gemma",
     "origin": "US / Google DeepMind"},
    # ── HF router additions ────────────────────────────────────────────────
    {"id": "qwen2.5-7b", "model": "qwen/qwen-2.5-7b-instruct",
     "provider": "openrouter", "license": "Qwen Community",
     "origin": "China / Alibaba (smaller model, L0 tasks)"},
]

DEFAULT_MODEL_ID = "deepseek-v3"


def find_rotation_entry(model_arg: str) -> dict:
    """Resolve --model argument to a rotation entry. Accepts short id or full model string.
    Raises SystemExit if not in the allowlist — prevents injection of arbitrary models."""
    for entry in ROTATION:
        if model_arg in (entry["id"], entry["model"]):
            return entry
    ids = ", ".join(e["id"] for e in ROTATION)
    print(f"Error: '{model_arg}' is not in the eligible model allowlist.", file=sys.stderr)
    print(f"Allowed model IDs: {ids}", file=sys.stderr)
    print("To add a model, open a PR against docs/ELIGIBLE-MODELS.md.", file=sys.stderr)
    sys.exit(1)


LEVEL_NAMES = {
    0: "L0 — Scaffolding (README, badges, CI config, docs)",
    1: "L1 — Language Foundations (Ehrenfest syntax, parser, AST, type system)",
    2: "L2 — Compiler / Afana (ZX-IR generation, rewriting rules, QASM3 output)",
    3: "L3 — Hardware Backends (IBM/IQM adapters, HAL Contract, error mitigation)",
    4: "L4 — Turing-Complete Runtime (quantum memory model, classical control flow)",
}

LABEL_TAXONOMY = "compiler · specification · infrastructure · agent-ux · docs · good-first-issue"

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
- Acceptance criteria: ≥2 bullet points, each CI-verifiable
  (a passing test, a file that exists, a command that succeeds)
- Label: exactly one from the taxonomy above

Output ONLY valid JSON in this exact structure — no prose before or after:

{{
  "title": "...",
  "description": "...",
  "acceptance_criteria": ["...", "..."],
  "label": "..."
}}"""


# ── Provider dispatch ─────────────────────────────────────────────────────────

def call_model(entry: dict, prompt: str) -> tuple[str, str | None]:
    """Call the model via its provider. Returns (response_text, verified_model_name).
    verified_model_name is non-None only when the provider confirms the served model."""
    provider_key = entry["provider"]
    provider = PROVIDERS[provider_key]
    api_key = os.environ.get(provider["env"], "").strip()
    if not api_key:
        print(f"Error: {provider['env']} not set (required for provider '{provider_key}').", file=sys.stderr)
        sys.exit(1)

    body = json.dumps({
        "model": entry["model"],
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 2048,
        "temperature": 0.7,
    }).encode()

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        **provider["headers"],
    }

    req = urllib.request.Request(provider["url"], data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=90) as resp:
            resp_headers = {k.lower(): v for k, v in resp.headers.items()}
            data = json.loads(resp.read())
    except urllib.error.HTTPError as e:
        body_text = e.read().decode()
        print(f"Provider '{provider_key}' error {e.code}: {body_text}", file=sys.stderr)
        sys.exit(1)

    text = data["choices"][0]["message"]["content"].strip()

    # Cross-check the served model name where the provider exposes it.
    # This catches silent re-routing to a different (possibly closed-weights) model.
    verify_header = provider.get("verify_header")
    verified_model = resp_headers.get(verify_header) if verify_header else None
    if verified_model and verified_model != entry["model"]:
        print(f"Warning: provider routed to '{verified_model}' instead of '{entry['model']}'.", file=sys.stderr)
        print("This may indicate silent model substitution. Check before publishing.", file=sys.stderr)

    return text, verified_model


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
*See [ISSUE-GENERATION.md](https://github.com/ehrenfest-quantum/quasi/blob/main/docs/ISSUE-GENERATION.md).*"""

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

def record_ledger(model: str, provider: str, level: int, issue_url: str) -> None:
    """Append a generation event to the quasi-ledger."""
    body = json.dumps({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "quasi:type": "issue_generated",
        "quasi:level": level,
        "quasi:generator_model": model,
        "quasi:generator_provider": provider,
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

  # Use a specific model (short ID or full API string)
  python3 quasi-agent/generate_issue.py --level 0 --model sarvam-m --dry-run
  python3 quasi-agent/generate_issue.py --level 0 --model mistral-nemo --dry-run
  python3 quasi-agent/generate_issue.py --level 0 --model deepseek-v3 --dry-run

  # Open a real GitHub issue (requires GITHUB_TOKEN)
  python3 quasi-agent/generate_issue.py --level 0

  # List eligible models
  python3 quasi-agent/generate_issue.py --list-models
""",
    )
    parser.add_argument("--level", type=int, default=0, choices=range(5),
                        help="Capability Ladder level to target (default: 0)")
    parser.add_argument("--model", default=None,
                        help=f"Model short ID or full API string (default: {DEFAULT_MODEL_ID}). "
                             f"Run --list-models to see all eligible IDs.")
    parser.add_argument("--dry-run", action="store_true",
                        help="Print the draft issue without opening it on GitHub")
    parser.add_argument("--list-models", action="store_true",
                        help="Print the eligible model rotation and exit")
    args = parser.parse_args()

    if args.list_models:
        print("\nEligible generation models:\n")
        print(f"  {'ID':<20} {'Provider':<12} {'License':<18} {'Origin'}")
        print(f"  {'-'*20} {'-'*12} {'-'*18} {'-'*30}")
        for e in ROTATION:
            print(f"  {e['id']:<20} {e['provider']:<12} {e['license']:<18} {e['origin']}")
        print()
        return

    # Resolve model → allowlist entry (exits if not eligible)
    model_arg = (
        args.model
        or os.environ.get("QUASI_GENERATOR_MODEL")
        or DEFAULT_MODEL_ID
    )
    entry = find_rotation_entry(model_arg)

    root = repo_root()
    level = args.level

    print("\n── QUASI Pauli-Test Issue Generator ──")
    print(f"   Model:    {entry['model']}")
    print(f"   Provider: {entry['provider']}  ({entry['origin']} · {entry['license']})")
    print(f"   Level:    L{level} — {LEVEL_NAMES[level]}")
    print(f"   Repo:     {root}")
    print(f"   Mode:     {'dry-run (no GitHub issue)' if args.dry_run else 'live'}")
    print()

    print("Building context...", end=" ", flush=True)
    prompt = build_context(level, root)
    print(f"done ({len(prompt)} chars)")

    print(f"Calling {entry['model']} via {entry['provider']}...", end=" ", flush=True)
    raw, verified_model = call_model(entry, prompt)
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
    issue_url = open_github_issue(issue, entry["model"], level, github_token)
    print(f"done\n  {issue_url}")

    print("Recording ledger entry...", end=" ", flush=True)
    record_ledger(entry["model"], entry["provider"], level, issue_url)

    print(f"\n✓ Issue generated at L{level} by {entry['model']} ({entry['provider']})")
    print(f"  {issue_url}")
    print()


if __name__ == "__main__":
    main()
