#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
rotate.py — Rotation scheduler for the QUASI Pauli-Test issue generator.

Picks the next model in round-robin order (model with fewest existing issues
per capability level) and calls generate_issue.py as a subprocess.

Usage:
    python3 quasi-agent/rotate.py              # auto-select model and level
    python3 quasi-agent/rotate.py --dry-run    # show selection, do not generate
    python3 quasi-agent/rotate.py --level 0    # fix the level, rotate model only

This script is designed to run as a systemd timer on Camelot.
Environment variables needed (same as generate_issue.py):
    GITHUB_TOKEN         Read issues + create issues
    OPENROUTER_API_KEY   OpenRouter models
    HF_TOKEN             HuggingFace Inference Router models
    SARVAM_API_KEY       Sarvam-30B/105B (India)
    MISTRAL_API_KEY      Mistral direct endpoint (optional)
    CSCS_SERVING_API     Swiss AI / Apertus (optional, needs account)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

# ── Import the rotation list from generate_issue.py ───────────────────────────
# rotate.py lives next to generate_issue.py; use the same ROTATION allowlist.

_here = Path(__file__).parent
sys.path.insert(0, str(_here))

from generate_issue import ROTATION, LEVEL_NAMES, PROVIDERS  # noqa: E402

GITHUB_API_BASE = "https://api.github.com/repos/ehrenfest-quantum/quasi"
ISSUES_PER_PAGE = 100
MAX_PAGES = 20  # cap at 2000 issues

# Planck quota — target issues per model per level.
# Named after Planck's constant (h ≈ 6.626 × 10⁻³⁴ J·s).
# When every eligible model has reached this count at every level,
# the rotation is complete and the scheduler exits without generating.
PLANCK_QUOTA = 6

# How to detect the generator model in an issue body.
# The generate_issue.py footer line is:
#   Generator model: `{model_id}` · Level: L{level}
GENERATOR_PATTERN = re.compile(
    r"Generator model:\s*`([^`]+)`\s*·\s*Level:\s*L(\d+)",
    re.IGNORECASE,
)

LOG_FILE = Path("/home/vops/quasi-rotate.log")


def log(msg: str) -> None:
    ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    line = f"[{ts}] {msg}"
    print(line, flush=True)
    try:
        with LOG_FILE.open("a") as f:
            f.write(line + "\n")
    except OSError:
        pass  # log file may not be writable in --dry-run on Mac


def github_get(path: str, token: str) -> list | dict:
    """GET from GitHub API, handling pagination for list endpoints."""
    results: list = []
    url: str | None = f"{GITHUB_API_BASE}{path}?per_page={ISSUES_PER_PAGE}&state=open"
    pages = 0
    while url and pages < MAX_PAGES:
        req = urllib.request.Request(
            url,
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            data = json.loads(resp.read().decode())
            if isinstance(data, list):
                results.extend(data)
                link = resp.headers.get("Link", "")
                # Parse next page URL from Link header
                next_url = None
                for part in link.split(","):
                    part = part.strip()
                    if 'rel="next"' in part:
                        m = re.search(r"<([^>]+)>", part)
                        if m:
                            next_url = m.group(1)
                url = next_url
            else:
                return data
        pages += 1
    return results


def _model_string_to_id() -> dict[str, str]:
    """Build a reverse map: full API model string -> short rotation ID."""
    return {e["model"]: e["id"] for e in ROTATION}


def count_issues_per_model_level(token: str) -> dict[str, dict[int, int]]:
    """
    Parse all open+closed QUASI issues and count how many each model has
    generated at each level.

    Returns: {short_id: {level: count}}
    """
    # Fetch open issues
    all_issues: list[dict] = []

    for state in ("open", "closed"):
        url: str | None = (
            f"{GITHUB_API_BASE}/issues"
            f"?per_page={ISSUES_PER_PAGE}&state={state}"
        )
        pages = 0
        while url and pages < MAX_PAGES:
            req = urllib.request.Request(
                url,
                headers={
                    "Authorization": f"Bearer {token}",
                    "Accept": "application/vnd.github+json",
                    "X-GitHub-Api-Version": "2022-11-28",
                },
            )
            try:
                with urllib.request.urlopen(req, timeout=20) as resp:
                    data = json.loads(resp.read().decode())
                    if isinstance(data, list):
                        all_issues.extend(data)
                        link = resp.headers.get("Link", "")
                        next_url = None
                        for part in link.split(","):
                            part = part.strip()
                            if 'rel="next"' in part:
                                m = re.search(r"<([^>]+)>", part)
                                if m:
                                    next_url = m.group(1)
                        url = next_url
                    else:
                        break
            except urllib.error.HTTPError as exc:
                log(f"GitHub API error {exc.code} fetching {state} issues")
                break
            pages += 1

    # Build reverse map so we can normalise the full model string in the issue
    # footer back to the short rotation ID used as the count key.
    model_str_to_id = _model_string_to_id()

    counts: dict[str, dict[int, int]] = defaultdict(lambda: defaultdict(int))
    matched = 0
    for issue in all_issues:
        body = issue.get("body") or ""
        m = GENERATOR_PATTERN.search(body)
        if m:
            raw_model = m.group(1)
            # Normalise: footer stores the full API model string; map to short id.
            # Falls back to the raw string so old issues with unknown models still count.
            model_id = model_str_to_id.get(raw_model, raw_model)
            level = int(m.group(2))
            counts[model_id][level] += 1
            matched += 1

    log(f"Scanned {len(all_issues)} issues, matched {matched} generator footers")
    return counts


def provider_has_key(provider_id: str) -> bool:
    """Return True if the required env var for this provider is set."""
    env_var = PROVIDERS.get(provider_id, {}).get("env")
    if not env_var:
        return True  # no key required
    return bool(os.environ.get(env_var))


def eligible_rotation() -> list[dict]:
    """Return only models whose provider's API key is available."""
    return [e for e in ROTATION if provider_has_key(e["provider"])]


def planck_quota_met(
    counts: dict[str, dict[int, int]],
    fixed_level: int | None = None,
) -> bool:
    """
    Return True if every eligible model has reached PLANCK_QUOTA issues
    at every relevant level. When True, the rotation is complete.
    """
    eligible = eligible_rotation()
    levels = [fixed_level] if fixed_level is not None else list(LEVEL_NAMES.keys())
    for entry in eligible:
        mid = entry["id"]
        for level in levels:
            if counts.get(mid, {}).get(level, 0) < PLANCK_QUOTA:
                return False
    return True


def pick_next(
    counts: dict[str, dict[int, int]],
    fixed_level: int | None = None,
) -> tuple[str, int]:
    """
    Pick (model_id, level) with the fewest generated issues.

    Strategy:
    1. For each (model_id, level) pair in eligible ROTATION × LEVEL_NAMES,
       compute issue count (0 if not in counts).
    2. Sort by count ascending, then by ROTATION index (stable ordering).
    3. Return the first pair below PLANCK_QUOTA.

    If fixed_level is given, only consider that level.
    """
    eligible = eligible_rotation()
    if not eligible:
        log("ERROR: No eligible models — no provider keys configured.")
        sys.exit(1)

    levels = [fixed_level] if fixed_level is not None else list(LEVEL_NAMES.keys())

    candidates: list[tuple[int, int, str, int]] = []  # (count, rotation_idx, model_id, level)
    for rot_idx, entry in enumerate(eligible):
        mid = entry["id"]
        for level in levels:
            c = counts.get(mid, {}).get(level, 0)
            if c < PLANCK_QUOTA:
                candidates.append((c, rot_idx, mid, level))

    candidates.sort()
    _, _, model_id, level = candidates[0]
    return model_id, level


def run_generator(model_id: str, level: int, dry_run: bool = False) -> int:
    """
    Call generate_issue.py as a subprocess.
    Returns the exit code.
    """
    script = _here / "generate_issue.py"
    cmd = [
        sys.executable,
        str(script),
        "--model", model_id,
        "--level", str(level),
    ]
    if dry_run:
        cmd.append("--dry-run")

    log(f"Invoking: {' '.join(cmd)}")
    result = subprocess.run(cmd, env=os.environ.copy())
    return result.returncode


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Rotate through QUASI eligible models and generate the next issue."
    )
    parser.add_argument("--dry-run", action="store_true",
                        help="Show selection and prompt, do not create a real issue.")
    parser.add_argument("--level", type=int, choices=list(LEVEL_NAMES.keys()),
                        help="Fix the capability level (0–4). Default: auto-select.")
    args = parser.parse_args()

    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        log("ERROR: GITHUB_TOKEN not set — cannot read issue counts from GitHub.")
        sys.exit(1)

    log("rotate.py starting — counting existing issues per model/level")
    try:
        counts = count_issues_per_model_level(token)
    except Exception as exc:
        log(f"ERROR counting issues: {exc}")
        sys.exit(1)

    # Print summary of current counts
    eligible = eligible_rotation()
    log(f"Eligible models in rotation: {len(eligible)}/{len(ROTATION)}")
    for entry in eligible:
        mid = entry["id"]
        level_counts = counts.get(mid, {})
        totals = " ".join(f"L{lv}:{level_counts.get(lv, 0)}" for lv in LEVEL_NAMES)
        log(f"  {mid:20s} {totals}")

    # Planck quota check — h ≈ 6.626: stop when all models reach 6 issues/level
    if planck_quota_met(counts, fixed_level=args.level):
        eligible = eligible_rotation()
        total = len(eligible) * len(LEVEL_NAMES) * PLANCK_QUOTA
        log(
            f"Planck quota met — all {len(eligible)} models have ≥{PLANCK_QUOTA} issues "
            f"at every level ({total} total). Rotation complete. Timer will keep firing "
            f"but do nothing until quota is raised or models are added."
        )
        return

    model_id, level = pick_next(counts, fixed_level=args.level)
    log(f"Selected: model={model_id} level=L{level} (quota={PLANCK_QUOTA}, h≈6.626)")

    rc = run_generator(model_id, level, dry_run=args.dry_run)
    if rc == 0:
        log(f"generate_issue.py exited OK for {model_id} L{level}")
    else:
        log(f"generate_issue.py exited with code {rc} for {model_id} L{level}")
        sys.exit(rc)


if __name__ == "__main__":
    main()
