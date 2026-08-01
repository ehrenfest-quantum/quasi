#!/usr/bin/env python3
"""Werner B-track Post-Mortem Evaluator.

Sibling to Werner's A-track shadow_evaluator. Where the A-track judges
issue-draft quality, this one performs a forensic review of *failed*
B-track attempts (B.1 solver json_fail / B.2 reviewer rejected) drawn
from `senate_telemetry`.

Output: a per-row failure category drawn from a fixed typology, plus a
short summary report aggregating the population. Read-only against the
senate; safe to run with senate timers stopped.

Usage:
    python3 werner_btrack_evaluator.py --batch 20    # analyse 20 rows
    python3 werner_btrack_evaluator.py --report      # summarise existing verdicts
    python3 werner_btrack_evaluator.py --since '2026-04-01'  # restrict window
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import re
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("werner-btrack")

BASE_DIR = Path("/home/vops/werner-shadow")
VERDICTS_DIR = BASE_DIR / "verdicts-btrack"
LOG_FILE = BASE_DIR / "verdict-log-btrack.jsonl"

DATABASE_URL = os.environ.get(
    "DATABASE_URL",
    "postgres://quasi_senate:qs_bench_2026@localhost/quasi_senate",
)
ANTHROPIC_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
OPENROUTER_API_KEY = os.environ.get("OPENROUTER_API_KEY", "")
GROQ_API_KEY = os.environ.get("GROQ_API_KEY", "")
MISTRAL_API_KEY = os.environ.get("MISTRAL_API_KEY", "")
TOGETHER_API_KEY = os.environ.get("TOGETHER_API_KEY", "")
GITHUB_TOKEN = os.environ.get("GITHUB_TOKEN", "")
GITHUB_REPO = os.environ.get("GITHUB_REPO", "ehrenfest-quantum/quasi")

# Canonical failure categories. Anything outside this set is normalised
# to "other" in post-processing.
CATEGORIES = [
    "parse_failure",            # malformed JSON / unparseable shape
    "schema_mismatch",          # valid JSON but wrong keys / structure
    "hallucinated_symbol",      # referenced functions/types/files that do not exist
    "off_target",               # patch was unrelated to the assigned issue
    "incomplete_implementation", # partial work that doesn't satisfy ACs
    "test_only_no_impl",        # added tests without implementation behind them
    "architectural_violation",  # broke CLAUDE.md invariants (Python in afana/, etc.)
    "domain_knowledge_gap",     # model lacks the physics/CBOR/ZX knowledge needed
    "hard_task",                # task itself appears beyond current model capability
    "other",
]

JUDGE_PROMPT = """You are a senior compiler engineer auditing FAILED attempts
by an autonomous AI senate to solve GitHub issues. Each row records one
solver or reviewer attempt that ended in failure (json parse failure, or
the reviewer rejected the solver's patch).

Your job: assign each failure to exactly ONE category from this fixed
taxonomy. Be decisive — pick the dominant cause.

Categories:
- parse_failure: malformed JSON / model returned non-JSON / truncated mid-string
- schema_mismatch: valid JSON but wrong keys / structure (e.g. returned
  {"name", "content"} when contract is {"reasoning", "edits", "new_files"})
- hallucinated_symbol: patch references functions/types/files that do not
  exist in the repo
- off_target: patch addressed something other than the assigned issue
  (classic example: fixing an unrelated CacheMetrics export instead of
  the requested ZX-IR work)
- incomplete_implementation: real attempt at the right area but doesn't
  satisfy the acceptance criteria (e.g. missing CBOR hex file when the
  AC required both .md and .cbor.hex)
- test_only_no_impl: added test fixtures without any implementation
  behind them
- architectural_violation: broke a stated invariant (e.g. introduced
  .py files in afana/ which is Rust-only by design)
- domain_knowledge_gap: model lacks specific knowledge needed (CBOR
  binary format, ZX-calculus algebra, Hamiltonian physics) — typically
  manifests as plausible-looking but technically wrong output
- hard_task: the task itself appears beyond current open-weight model
  capability irrespective of which model is used
- other: doesn't fit anywhere above

Return ONLY a JSON object:
{
  "category": "<one of the above>",
  "confidence": <0.0-1.0>,
  "key_quote": "<one short verbatim phrase from the input that drove the verdict>",
  "reasoning": "<one sentence explaining why this category>"
}
"""


# ---------------------------------------------------------------------------
# Postgres (stdlib via subprocess — no psycopg2 dep needed)
# ---------------------------------------------------------------------------

def _psql(query: str) -> list[dict[str, Any]]:
    """Run a query, return rows as list of dicts. Uses JSON output via psql."""
    import subprocess

    # Wrap query so psql emits jsonb array
    wrapped = (
        "SELECT coalesce(json_agg(row_to_json(t)), '[]'::json) "
        f"FROM ({query}) t"
    )
    cmd = [
        "psql", DATABASE_URL,
        "-At",  # tuples only, no align
        "-c", wrapped,
    ]
    out = subprocess.check_output(cmd, timeout=60)
    return json.loads(out.decode().strip() or "[]")


# ---------------------------------------------------------------------------
# Anthropic LLM call
# ---------------------------------------------------------------------------

def call_anthropic(prompt: str) -> Optional[str]:
    """Single Anthropic Haiku call. Returns content or None on failure."""
    if not ANTHROPIC_API_KEY:
        return None

    payload = json.dumps({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 512,
        "messages": [{"role": "user", "content": prompt}],
    }).encode("utf-8")

    req = urllib.request.Request(
        "https://api.anthropic.com/v1/messages",
        data=payload,
        headers={
            "x-api-key": ANTHROPIC_API_KEY,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
        return data["content"][0]["text"]
    except urllib.error.HTTPError as e:
        log.error("Anthropic HTTP %s: %s", e.code, e.read()[:200])
        return None
    except Exception as e:
        log.exception("Anthropic call failed: %s", e)
        return None


def call_openrouter(prompt: str, model: str = "deepseek/deepseek-r1") -> Optional[str]:
    """OpenRouter call via OpenAI-compat endpoint."""
    if not OPENROUTER_API_KEY:
        return None

    payload = json.dumps({
        "model": model,
        "max_tokens": 512,
        "temperature": 0.1,
        "messages": [{"role": "user", "content": prompt}],
    }).encode("utf-8")

    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions",
        data=payload,
        headers={
            "Authorization": f"Bearer {OPENROUTER_API_KEY}",
            "HTTP-Referer": "https://quasi.arvak.io",
            "X-Title": "Werner B-track Post-Mortem",
            "Content-Type": "application/json",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            data = json.loads(resp.read().decode())
        return data["choices"][0]["message"]["content"]
    except urllib.error.HTTPError as e:
        log.error("OpenRouter HTTP %s: %s", e.code, e.read()[:200])
        return None
    except Exception as e:
        log.exception("OpenRouter call failed: %s", e)
        return None


def call_groq(prompt: str, model: str = "llama-3.3-70b-versatile") -> Optional[str]:
    """Groq inference. Fast + free tier for our scale."""
    if not GROQ_API_KEY:
        return None

    payload = json.dumps({
        "model": model,
        "max_tokens": 512,
        "temperature": 0.1,
        "messages": [{"role": "user", "content": prompt}],
    }).encode("utf-8")

    req = urllib.request.Request(
        "https://api.groq.com/openai/v1/chat/completions",
        data=payload,
        headers={
            "Authorization": f"Bearer {GROQ_API_KEY}",
            "Content-Type": "application/json",
            "User-Agent": "werner-btrack/1.0",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
        return data["choices"][0]["message"]["content"]
    except urllib.error.HTTPError as e:
        log.error("Groq HTTP %s: %s", e.code, e.read()[:200])
        return None
    except Exception as e:
        log.exception("Groq call failed: %s", e)
        return None


def call_mistral(prompt: str, model: str = "mistral-large-latest") -> Optional[str]:
    """Mistral platform call."""
    if not MISTRAL_API_KEY:
        return None
    payload = json.dumps({
        "model": model,
        "max_tokens": 512,
        "temperature": 0.1,
        "messages": [{"role": "user", "content": prompt}],
    }).encode("utf-8")
    req = urllib.request.Request(
        "https://api.mistral.ai/v1/chat/completions",
        data=payload,
        headers={
            "Authorization": f"Bearer {MISTRAL_API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
        return data["choices"][0]["message"]["content"]
    except urllib.error.HTTPError as e:
        log.error("Mistral HTTP %s: %s", e.code, e.read()[:200])
        return None
    except Exception as e:
        log.exception("Mistral call failed: %s", e)
        return None


def call_together(prompt: str, model: str = "deepseek-ai/DeepSeek-V3") -> Optional[str]:
    """Together.ai call."""
    if not TOGETHER_API_KEY:
        return None
    payload = json.dumps({
        "model": model,
        "max_tokens": 512,
        "temperature": 0.1,
        "messages": [{"role": "user", "content": prompt}],
    }).encode("utf-8")
    req = urllib.request.Request(
        "https://api.together.xyz/v1/chat/completions",
        data=payload,
        headers={
            "Authorization": f"Bearer {TOGETHER_API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
        return data["choices"][0]["message"]["content"]
    except urllib.error.HTTPError as e:
        log.error("Together HTTP %s: %s", e.code, e.read()[:200])
        return None
    except Exception as e:
        log.exception("Together call failed: %s", e)
        return None


def call_judge(prompt: str) -> Optional[str]:
    """Try providers in order. First success wins.

    Ordered by empirical reviewer strength (per senate_telemetry B2_reviewer
    data): deepseek-v3 / deepseek-r1 (best critics) → llama-3.3-70b → mistral.
    """
    for caller in (
        lambda p: call_anthropic(p),
        lambda p: call_together(p, "deepseek-ai/DeepSeek-V3"),
        lambda p: call_groq(p, "llama-3.3-70b-versatile"),
        lambda p: call_mistral(p, "mistral-large-latest"),
        lambda p: call_openrouter(p, "deepseek/deepseek-r1"),
    ):
        result = caller(prompt)
        if result:
            return result
    log.error("All judge providers failed")
    return None


def parse_verdict(text: str) -> Optional[dict[str, Any]]:
    """Extract the JSON verdict. Returns None on failure."""
    match = re.search(r"\{[\s\S]*\}", text)
    if not match:
        return None
    try:
        data = json.loads(match.group())
    except json.JSONDecodeError:
        return None

    category = str(data.get("category", "")).lower().replace(" ", "_").replace("-", "_")
    if category not in CATEGORIES:
        category = "other"

    return {
        "category": category,
        "confidence": float(data.get("confidence", 0.5)),
        "key_quote": str(data.get("key_quote", ""))[:300],
        "reasoning": str(data.get("reasoning", ""))[:500],
    }


# ---------------------------------------------------------------------------
# Context assembly
# ---------------------------------------------------------------------------

@dataclass
class FailureRow:
    telemetry_id: int
    timestamp: str
    role: str
    model_id: str
    provider: str
    issue_number: Optional[int]
    downstream_verdict: str
    verdict_reasoning: str
    issue_title: str
    issue_body: str
    raw_response_excerpt: str

    def to_prompt(self) -> str:
        return f"""{JUDGE_PROMPT}

---
TELEMETRY ROW {self.telemetry_id}
- timestamp: {self.timestamp}
- role: {self.role}
- model: {self.model_id} (provider: {self.provider})
- outcome: {self.downstream_verdict}
- issue: #{self.issue_number or "?"}

ISSUE TITLE: {self.issue_title}

ISSUE BODY (truncated):
{self.issue_body[:1500]}

REVIEWER OR PARSER FEEDBACK:
{self.verdict_reasoning[:1500] if self.verdict_reasoning else "(none)"}

RAW MODEL OUTPUT EXCERPT (if json_fail):
{self.raw_response_excerpt[:1500] if self.raw_response_excerpt else "(not preserved)"}
---
"""


def fetch_issue(issue_number: int) -> tuple[str, str]:
    """Fetch issue title/body from GitHub. Returns ('', '') on failure."""
    if not issue_number:
        return "", ""
    url = f"https://api.github.com/repos/{GITHUB_REPO}/issues/{issue_number}"
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "werner-btrack/1.0",
    }
    if GITHUB_TOKEN:
        headers["Authorization"] = f"Bearer {GITHUB_TOKEN}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode())
        return data.get("title", ""), data.get("body", "") or ""
    except Exception as e:
        log.warning("issue #%d fetch failed: %s", issue_number, e)
        return "", ""


def fetch_raw_dump(model_id: str, issue_number: Optional[int]) -> str:
    """Best-effort: read the /tmp/senate-raw-{model}-{issue}.txt dump if present."""
    if not issue_number:
        return ""
    path = Path(f"/tmp/senate-raw-{model_id}-{issue_number}.txt")
    if not path.exists():
        return ""
    try:
        return path.read_text(errors="replace")[:2000]
    except OSError:
        return ""


def sample_candidates(limit: int, since: Optional[str]) -> list[dict[str, Any]]:
    """Pull failed telemetry rows, weighted to mix json_fail + rejected."""
    where_since = f"AND timestamp >= '{since}'" if since else ""
    query = f"""
        SELECT id, timestamp::text AS timestamp, role, model_id, provider,
               issue_number, downstream_verdict,
               coalesce(verdict_reasoning, '') AS verdict_reasoning
        FROM senate_telemetry
        WHERE downstream_verdict IN ('rejected', 'json_fail')
          {where_since}
        ORDER BY random()
        LIMIT {int(limit)}
    """
    return _psql(query)


# ---------------------------------------------------------------------------
# Evaluation loop
# ---------------------------------------------------------------------------

def run_batch(limit: int, since: Optional[str], delay: float = 2.0) -> None:
    VERDICTS_DIR.mkdir(parents=True, exist_ok=True)

    log.info("Sampling %d failed telemetry rows (since=%s)", limit, since or "all-time")
    candidates = sample_candidates(limit, since)
    log.info("Got %d candidates", len(candidates))

    # Skip already-evaluated rows
    already = {p.stem for p in VERDICTS_DIR.glob("*.json")}
    todo = [r for r in candidates if str(r["id"]) not in already]
    log.info("%d remain after de-dup", len(todo))

    for i, row in enumerate(todo, 1):
        issue_title, issue_body = fetch_issue(row["issue_number"])
        raw = fetch_raw_dump(row["model_id"], row["issue_number"])

        f = FailureRow(
            telemetry_id=row["id"],
            timestamp=row["timestamp"],
            role=row["role"],
            model_id=row["model_id"],
            provider=row["provider"],
            issue_number=row["issue_number"],
            downstream_verdict=row["downstream_verdict"],
            verdict_reasoning=row["verdict_reasoning"],
            issue_title=issue_title,
            issue_body=issue_body,
            raw_response_excerpt=raw,
        )

        log.info("[%d/%d] judging telemetry id=%d (%s, issue=#%s, model=%s)",
                 i, len(todo), f.telemetry_id, f.downstream_verdict,
                 f.issue_number, f.model_id)
        raw_resp = call_judge(f.to_prompt())
        if not raw_resp:
            log.warning("LLM call failed for telemetry id=%d", f.telemetry_id)
            continue

        verdict = parse_verdict(raw_resp)
        if not verdict:
            log.warning("Could not parse verdict for telemetry id=%d", f.telemetry_id)
            continue

        record = {
            "evaluated_at": datetime.now(timezone.utc).isoformat(),
            "telemetry_id": f.telemetry_id,
            "telemetry_timestamp": f.timestamp,
            "role": f.role,
            "model_id": f.model_id,
            "provider": f.provider,
            "issue_number": f.issue_number,
            "downstream_verdict": f.downstream_verdict,
            **verdict,
        }

        (VERDICTS_DIR / f"{f.telemetry_id}.json").write_text(
            json.dumps(record, indent=2)
        )
        with open(LOG_FILE, "a") as fh:
            fh.write(json.dumps(record) + "\n")

        time.sleep(delay)


def report() -> None:
    """Aggregate existing verdicts into a typology summary."""
    if not VERDICTS_DIR.exists():
        print("No verdicts yet — run --batch first.")
        return

    records = []
    for p in VERDICTS_DIR.glob("*.json"):
        try:
            records.append(json.loads(p.read_text()))
        except json.JSONDecodeError:
            continue

    if not records:
        print("Verdict dir is empty.")
        return

    print(f"\n=== Werner B-track Post-Mortem Report ({len(records)} failures judged) ===\n")

    # Category breakdown
    from collections import Counter, defaultdict
    by_category = Counter(r["category"] for r in records)
    print("Failure categories (overall):")
    for cat, n in by_category.most_common():
        pct = 100.0 * n / len(records)
        print(f"  {cat:32s} {n:4d}  ({pct:5.1f}%)")

    # Per-role breakdown
    print("\nBy role:")
    by_role: dict[str, Counter[str]] = defaultdict(Counter)
    for r in records:
        by_role[r["role"]][r["category"]] += 1
    for role, counts in sorted(by_role.items()):
        total = sum(counts.values())
        top = counts.most_common(3)
        top_str = ", ".join(f"{c}={n}" for c, n in top)
        print(f"  {role}  (n={total}): {top_str}")

    # Per-model top failure
    print("\nTop failure mode per model (only models with >= 5 failures):")
    by_model: dict[str, Counter[str]] = defaultdict(Counter)
    for r in records:
        by_model[r["model_id"]][r["category"]] += 1
    for model, counts in sorted(by_model.items(), key=lambda kv: -sum(kv[1].values())):
        n = sum(counts.values())
        if n < 5:
            continue
        top_cat, top_n = counts.most_common(1)[0]
        print(f"  {model:32s} n={n:3d}  top={top_cat} ({top_n}/{n})")

    # Confidence distribution
    confs = [r["confidence"] for r in records]
    avg_conf = sum(confs) / len(confs) if confs else 0
    print(f"\nMean judge confidence: {avg_conf:.2f}")

    # Representative quotes per category (top 3)
    print("\nRepresentative key quotes per category (one each):")
    seen_cats: set[str] = set()
    for r in sorted(records, key=lambda r: -r["confidence"]):
        cat = r["category"]
        if cat in seen_cats:
            continue
        seen_cats.add(cat)
        quote = r["key_quote"][:120]
        print(f"  [{cat}] {quote!r}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    p = argparse.ArgumentParser(description="Werner B-track post-mortem evaluator")
    p.add_argument("--batch", type=int, default=0, help="number of rows to evaluate")
    p.add_argument("--since", type=str, default=None, help="ISO date — only rows >= this")
    p.add_argument("--report", action="store_true", help="print aggregate report")
    p.add_argument("--delay", type=float, default=2.0, help="sleep between LLM calls (sec)")
    args = p.parse_args()

    if args.batch > 0:
        if not (ANTHROPIC_API_KEY or OPENROUTER_API_KEY or GROQ_API_KEY):
            print(
                "ERROR: need ANTHROPIC_API_KEY or OPENROUTER_API_KEY or GROQ_API_KEY",
                file=sys.stderr,
            )
            sys.exit(1)
        run_batch(args.batch, args.since, delay=args.delay)

    if args.report or args.batch == 0:
        report()


if __name__ == "__main__":
    main()
