#!/usr/bin/env python3
"""Shadow Judge — batch issue-quality rubric over the collector queue.

NOT Werner. Werner is a fine-tuned quantum-first gatekeeper whose taxonomy is
quantum-first / classical-contaminated / reformulate. This module applies a
generic specification-quality rubric using a generic open-weight model, and
keeps the tamper-evident hash chain around the result.

See quasi-senate/werner/README.md for the distinction.

Consumes queue.jsonl entries written by shadow_collector.py,
evaluates each issue via LLM, stores cleartext verdicts with
tamper-evident SHA-256 hash chain.

Runs on Camelot. Stdlib only (urllib.request, no httpx).

Usage:
    python3 shadow_evaluator.py --batch 5        # evaluate next 5
    python3 shadow_evaluator.py --backfill       # all unevaluated (6s delay)
    python3 shadow_evaluator.py --verify         # check chain integrity
    python3 shadow_evaluator.py --status         # queue/chain stats
"""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import re
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("shadow-judge")

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

BASE_DIR = Path("/home/vops/shadow-judge")
QUEUE_FILE = BASE_DIR / "queue.jsonl"
CHAIN_FILE = BASE_DIR / "chain.jsonl"
VERDICT_LOG = BASE_DIR / "verdict-log.jsonl"
VERDICTS_DIR = BASE_DIR / "verdicts"

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------

GITHUB_TOKEN = os.environ.get("GITHUB_TOKEN", "")
GITHUB_REPO = os.environ.get("GITHUB_REPO", "ehrenfest-quantum/quasi")
OPENROUTER_API_KEY = os.environ.get("OPENROUTER_API_KEY", "")
GROQ_API_KEY = os.environ.get("GROQ_API_KEY", "")

# ---------------------------------------------------------------------------
# Judge prompt
# ---------------------------------------------------------------------------

JUDGE_SYSTEM = """You are a senior software engineering issue quality evaluator.
Assess the given GitHub issue for clarity, actionability, and difficulty.

Return a JSON object with exactly these fields:
{
  "verdict": "<well-specified|underspecified|ambiguous|trivial|unsolvable>",
  "confidence": <0.0-1.0>,
  "difficulty_tier": <1-5>,
  "signals": ["list", "of", "quality", "signals"],
  "reasoning": "One paragraph explaining the verdict."
}

Verdict definitions:
- well-specified: Clear problem, clear acceptance criteria, implementable
- underspecified: Missing key details needed to implement (unclear scope, missing context)
- ambiguous: Could be interpreted multiple ways, contradictory requirements
- trivial: Too simple to be meaningful (typo fix, one-line change)
- unsolvable: Fundamentally impossible or contradictory

Difficulty tiers:
1 = trivial (< 30 min), 2 = easy (< 2h), 3 = medium (half day),
4 = hard (1-3 days), 5 = very hard (> 3 days)

Return ONLY the JSON object, no markdown fences, no extra text."""

# ---------------------------------------------------------------------------
# Hash chain
# ---------------------------------------------------------------------------


def sha256(data: str) -> str:
    return hashlib.sha256(data.encode("utf-8")).hexdigest()


def get_last_chain_entry() -> dict[str, Any] | None:
    if not CHAIN_FILE.exists():
        return None
    lines = CHAIN_FILE.read_text().strip().split("\n")
    if not lines or not lines[-1].strip():
        return None
    return json.loads(lines[-1])


def append_chain_entry(
    seq: int, issue_number: int, verdict_json: str, prev_hash: str
) -> dict[str, Any]:
    verdict_hash = sha256(verdict_json)
    chain_hash = sha256(verdict_hash + prev_hash)
    entry = {
        "seq": seq,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "issue_number": issue_number,
        "verdict_hash": verdict_hash,
        "prev_hash": prev_hash,
        "chain_hash": chain_hash,
    }
    with open(CHAIN_FILE, "a") as f:
        f.write(json.dumps(entry) + "\n")
    return entry


def verify_chain() -> tuple[bool, list[str]]:
    if not CHAIN_FILE.exists():
        return False, ["Chain file not found"]
    errors: list[str] = []
    lines = CHAIN_FILE.read_text().strip().split("\n")
    prev_hash = "genesis"
    for i, line in enumerate(lines):
        if not line.strip():
            continue
        entry = json.loads(line)
        if entry["seq"] != i:
            errors.append(f"Seq mismatch at line {i}: expected {i}, got {entry['seq']}")
        if entry["prev_hash"] != prev_hash:
            errors.append(f"Chain break at seq {entry['seq']}: prev_hash mismatch")
        expected = sha256(entry["verdict_hash"] + entry["prev_hash"])
        if entry["chain_hash"] != expected:
            errors.append(f"Chain hash invalid at seq {entry['seq']}")
        verdict_file = VERDICTS_DIR / f"{entry['issue_number']}.json"
        if verdict_file.exists():
            actual = sha256(verdict_file.read_text())
            if actual != entry["verdict_hash"]:
                errors.append(
                    f"SEAL BROKEN: verdict #{entry['issue_number']} "
                    f"modified (hash mismatch at seq {entry['seq']})"
                )
        else:
            errors.append(f"Verdict file missing: #{entry['issue_number']}")
        prev_hash = entry["chain_hash"]
    return len(errors) == 0, errors


# ---------------------------------------------------------------------------
# GitHub fetch
# ---------------------------------------------------------------------------


def fetch_issue(issue_number: int) -> dict[str, str] | None:
    url = f"https://api.github.com/repos/{GITHUB_REPO}/issues/{issue_number}"
    req = urllib.request.Request(url, headers={
        "Authorization": f"token {GITHUB_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
        "User-Agent": "shadow-judge/1.0",
    })
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode())
        return {"title": data.get("title", ""), "body": data.get("body", "") or ""}
    except Exception as e:
        log.error("GitHub fetch #%d failed: %s", issue_number, e)
        return None


# ---------------------------------------------------------------------------
# LLM providers (fallback chain)
# ---------------------------------------------------------------------------


def _api_call(url: str, headers: dict, payload: dict, timeout: int = 60) -> str:
    """Generic API call, returns assistant message content."""
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        result = json.loads(resp.read().decode())
    return result["choices"][0]["message"]["content"]


def call_openrouter(issue_text: str) -> str | None:
    if not OPENROUTER_API_KEY:
        return None
    try:
        return _api_call(
            "https://openrouter.ai/api/v1/chat/completions",
            {
                "Authorization": f"Bearer {OPENROUTER_API_KEY}",
                "Content-Type": "application/json",
            },
            {
                "model": "microsoft/phi-4",  # open-weight; disjoint from generator pool
                "messages": [
                    {"role": "system", "content": JUDGE_SYSTEM},
                    {"role": "user", "content": issue_text},
                ],
                "max_tokens": 512,
                "temperature": 0.2,
            },
        )
    except Exception as e:
        log.warning("OpenRouter failed: %s", e)
        return None


def call_groq(issue_text: str) -> str | None:
    if not GROQ_API_KEY:
        return None
    try:
        return _api_call(
            "https://api.groq.com/openai/v1/chat/completions",
            {
                "Authorization": f"Bearer {GROQ_API_KEY}",
                "Content-Type": "application/json",
                "User-Agent": "quasi-agent/1.0 (https://quasi.arvak.io)",
            },
            {
                "model": "llama-3.3-70b-versatile",
                "messages": [
                    {"role": "system", "content": JUDGE_SYSTEM},
                    {"role": "user", "content": issue_text},
                ],
                "max_tokens": 512,
                "temperature": 0.2,
            },
        )
    except Exception as e:
        log.warning("Groq failed: %s", e)
        return None


def judge_issue(title: str, body: str) -> tuple[dict[str, Any], str, str]:
    """Call LLM with fallback chain. Returns (parsed, raw, provider)."""
    issue_text = f"# {title}\n\n{body}"

    # Open-weight judges only (project rule). The Anthropic leg was removed:
    # This judges an automated pipeline, so a commercial closed-weight model
    # is not permitted here. Groq leads because llama-3.3-70b-versatile is fast
    # and free-tier; phi-4 on OpenRouter is the fallback. Both are open-weight
    # and neither shares a model family with any active senate generator.
    for name, fn in [("groq", call_groq), ("openrouter", call_openrouter)]:
        raw = fn(issue_text)
        if raw:
            parsed = parse_verdict(raw)
            if parsed:
                return parsed, raw, name
            log.warning("Provider %s returned unparseable response", name)

    return {
        "verdict": "unparseable",
        "confidence": 0.0,
        "difficulty_tier": 0,
        "signals": [],
        "reasoning": "All providers failed or returned unparseable responses",
    }, "", "none"


def parse_verdict(text: str) -> dict[str, Any] | None:
    try:
        # Find JSON object in response
        match = re.search(r"\{[^{}]*\}", text, re.DOTALL)
        if not match:
            return None
        data = json.loads(match.group())
        verdict = data.get("verdict", "").lower().replace(" ", "-")
        valid = {"well-specified", "underspecified", "ambiguous", "trivial", "unsolvable"}
        if verdict not in valid:
            return None
        return {
            "verdict": verdict,
            "confidence": float(data.get("confidence", 0.5)),
            "difficulty_tier": int(data.get("difficulty_tier", 3)),
            "signals": data.get("signals", []),
            "reasoning": data.get("reasoning", ""),
        }
    except (json.JSONDecodeError, ValueError, TypeError):
        return None


# ---------------------------------------------------------------------------
# Queue operations
# ---------------------------------------------------------------------------


def load_queue() -> list[dict[str, Any]]:
    if not QUEUE_FILE.exists():
        return []
    entries = []
    for line in QUEUE_FILE.read_text().strip().split("\n"):
        if line.strip():
            entries.append(json.loads(line))
    return entries


def save_queue(entries: list[dict[str, Any]]) -> None:
    with open(QUEUE_FILE, "w") as f:
        for e in entries:
            f.write(json.dumps(e) + "\n")
    # Also sync shadow-queue.jsonl if it exists
    shadow = BASE_DIR / "shadow-queue.jsonl"
    if shadow.exists():
        with open(shadow, "w") as f:
            for e in entries:
                f.write(json.dumps(e) + "\n")


def get_unevaluated(queue: list[dict[str, Any]]) -> list[int]:
    """Return indices of unevaluated entries."""
    return [i for i, e in enumerate(queue) if not e.get("evaluated")]


# ---------------------------------------------------------------------------
# Core evaluation
# ---------------------------------------------------------------------------


def evaluate_batch(limit: int | None = None, delay: float = 6.0) -> int:
    """Evaluate unevaluated queue entries. Returns count processed."""
    VERDICTS_DIR.mkdir(parents=True, exist_ok=True)

    queue = load_queue()
    unevaluated = get_unevaluated(queue)

    if not unevaluated:
        log.info("No unevaluated entries in queue")
        return 0

    if limit:
        unevaluated = unevaluated[:limit]

    log.info("Evaluating %d issues (of %d pending)", len(unevaluated), len(get_unevaluated(queue)))

    # Chain state
    last = get_last_chain_entry()
    seq = (last["seq"] + 1) if last else 0
    prev_hash = last["chain_hash"] if last else "genesis"

    processed = 0
    for idx in unevaluated:
        entry = queue[idx]
        issue_number = entry["issue_number"]

        # Fetch from GitHub
        issue = fetch_issue(issue_number)
        if not issue:
            log.warning("Skipping #%d — could not fetch", issue_number)
            continue

        log.info("Judging #%d: %s", issue_number, issue["title"][:60])

        # Judge
        parsed, raw, provider = judge_issue(issue["title"], issue["body"])

        # Build verdict document
        verdict_doc = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "issue_number": issue_number,
            "issue_title": issue["title"],
            "verdict": parsed["verdict"],
            "confidence": parsed["confidence"],
            "difficulty_tier": parsed["difficulty_tier"],
            "signals": parsed["signals"],
            "reasoning": parsed["reasoning"],
            "provider": provider,
        }
        verdict_json = json.dumps(verdict_doc, ensure_ascii=False)

        # 1. Write verdict file (cleartext)
        (VERDICTS_DIR / f"{issue_number}.json").write_text(verdict_json)

        # 2. Append to hash chain
        chain_entry = append_chain_entry(seq, issue_number, verdict_json, prev_hash)
        seq += 1
        prev_hash = chain_entry["chain_hash"]

        # 3. Append to verdict log (flat JSONL for ETL)
        with open(VERDICT_LOG, "a") as f:
            f.write(verdict_json + "\n")

        # 4. Mark evaluated in queue
        queue[idx]["evaluated"] = True
        queue[idx]["evaluated_at"] = datetime.now(timezone.utc).isoformat()
        save_queue(queue)

        log.info(
            "  → %s (%.2f) tier=%d provider=%s chain=%s",
            parsed["verdict"], parsed["confidence"],
            parsed["difficulty_tier"], provider,
            chain_entry["chain_hash"][:16] + "...",
        )

        processed += 1
        if processed < len(unevaluated):
            time.sleep(delay)

    log.info("Done. Evaluated %d issues.", processed)
    return processed


# ---------------------------------------------------------------------------
# Status
# ---------------------------------------------------------------------------


def show_status() -> None:
    queue = load_queue()
    evaluated = sum(1 for e in queue if e.get("evaluated"))
    pending = len(queue) - evaluated

    print(f"Queue: {len(queue)} total, {pending} pending, {evaluated} evaluated")

    if CHAIN_FILE.exists():
        lines = [l for l in CHAIN_FILE.read_text().strip().split("\n") if l.strip()]
        print(f"Chain: {len(lines)} entries")
        if lines:
            last = json.loads(lines[-1])
            print(f"  Last: seq={last['seq']} issue=#{last['issue_number']} at {last['timestamp']}")
    else:
        print("Chain: not started")

    if VERDICT_LOG.exists():
        vlines = [l for l in VERDICT_LOG.read_text().strip().split("\n") if l.strip()]
        print(f"Verdict log: {len(vlines)} entries")
    else:
        print("Verdict log: empty")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description="Shadow Judge — issue-quality rubric (not Werner)")
    parser.add_argument("--batch", type=int, default=5, help="Evaluate N issues (default: 5)")
    parser.add_argument("--backfill", action="store_true", help="Evaluate ALL unevaluated (6s delay)")
    parser.add_argument("--verify", action="store_true", help="Verify chain integrity")
    parser.add_argument("--status", action="store_true", help="Show queue/chain stats")
    parser.add_argument("--delay", type=float, default=6.0, help="Delay between evaluations (seconds)")
    args = parser.parse_args()

    if args.status:
        show_status()
        return

    if args.verify:
        ok, errors = verify_chain()
        if ok:
            print("CHAIN INTACT — no tampering detected")
        else:
            print("SEAL STATUS:")
            for err in errors:
                print(f"  {err}")
        sys.exit(0 if ok else 1)

    # Need at least GitHub token
    if not GITHUB_TOKEN:
        log.error("GITHUB_TOKEN not set")
        sys.exit(1)

    # Need at least one LLM provider
    if not any([GROQ_API_KEY, OPENROUTER_API_KEY]):
        log.error("No LLM API key set (need GROQ_API_KEY or OPENROUTER_API_KEY)")
        sys.exit(1)

    limit = None if args.backfill else args.batch
    count = evaluate_batch(limit=limit, delay=args.delay)

    if count == 0:
        log.info("Nothing to evaluate.")


if __name__ == "__main__":
    main()
