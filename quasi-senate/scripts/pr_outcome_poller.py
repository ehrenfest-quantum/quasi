#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
PR Outcome Poller — updates pr_outcomes rows with live CI + merge state.

Reads GITHUB_TOKEN, GITHUB_REPO, DATABASE_URL from env (or /home/vops/.env.quasi).
Polls every RUN_INTERVAL_SECONDS (default 600) for PRs where merged IS NULL.
Designed to run as a systemd one-shot triggered by a timer.

Exit codes:
  0 — completed normally
  1 — fatal configuration error (DATABASE_URL or GITHUB_TOKEN missing)
"""

import os
import sys
import time
import logging
import re
from datetime import datetime, timezone
from pathlib import Path

import requests
import psycopg2

# ── Logging ──────────────────────────────────────────────────────────────────

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(message)s",
    datefmt="%Y-%m-%dT%H:%M:%SZ",
)
log = logging.getLogger("pr_outcome_poller")


# ── Config ───────────────────────────────────────────────────────────────────

def load_dotenv(path: str) -> None:
    """Load KEY=VALUE pairs from a file into os.environ (no-op if missing)."""
    try:
        for line in Path(path).read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))
    except FileNotFoundError:
        pass


load_dotenv("/home/vops/.env.quasi")

GITHUB_TOKEN = os.environ.get("GITHUB_TOKEN", "")
GITHUB_REPO = os.environ.get("GITHUB_REPO", "ehrenfest-quantum/quasi")
DATABASE_URL = os.environ.get("DATABASE_URL", "")


def _gh_headers() -> dict:
    return {
        "Authorization": f"Bearer {GITHUB_TOKEN}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }


# ── GitHub helpers ────────────────────────────────────────────────────────────

def get_pr(pr_number: int) -> dict | None:
    """Fetch PR metadata (state, merged_at, merge_commit_sha)."""
    url = f"https://api.github.com/repos/{GITHUB_REPO}/pulls/{pr_number}"
    resp = requests.get(url, headers=_gh_headers(), timeout=15)
    if resp.status_code == 404:
        return None
    resp.raise_for_status()
    return resp.json()


def get_check_runs(pr_number: int, head_sha: str) -> str:
    """
    Return a normalised CI status string for the PR's head commit:
      "passing"  — all required checks passed (or no checks found)
      "failing"  — at least one check failed/cancelled
      "error"    — at least one check errored
      "pending"  — checks still running / queued
    """
    url = f"https://api.github.com/repos/{GITHUB_REPO}/commits/{head_sha}/check-runs"
    resp = requests.get(url, headers=_gh_headers(), timeout=15, params={"per_page": 100})
    if resp.status_code in (404, 422):
        # No checks configured — treat as passing
        return "passing"
    resp.raise_for_status()

    runs = resp.json().get("check_runs", [])
    if not runs:
        return "passing"

    statuses = {r["conclusion"] for r in runs if r["status"] == "completed"}
    in_progress = [r for r in runs if r["status"] != "completed"]

    if in_progress:
        return "pending"
    if "failure" in statuses or "cancelled" in statuses or "timed_out" in statuses:
        return "failing"
    if "action_required" in statuses or "stale" in statuses:
        return "error"
    # success / neutral / skipped
    return "passing"


def pr_number_from_url(pr_url: str) -> int | None:
    """Extract PR number from a GitHub PR URL."""
    m = re.search(r"/pull/(\d+)$", pr_url)
    return int(m.group(1)) if m else None


# ── DB helpers ────────────────────────────────────────────────────────────────

FETCH_SQL = """
    SELECT id, pr_url, pr_number
    FROM pr_outcomes
    WHERE merged IS NULL
    ORDER BY created_at
    LIMIT 50
"""

UPDATE_SQL = """
    UPDATE pr_outcomes
    SET ci_status     = %s,
        ci_checked_at = %s,
        merged        = %s,
        merged_at     = %s
    WHERE id = %s
"""


# ── Main poll loop ────────────────────────────────────────────────────────────

def poll_once(conn) -> int:
    """Poll all unresolved PRs once. Returns the number of rows updated."""
    now = datetime.now(tz=timezone.utc)

    with conn.cursor() as cur:
        cur.execute(FETCH_SQL)
        rows = cur.fetchall()

    if not rows:
        log.info("poll_once: no unresolved PRs")
        return 0

    updated = 0
    for row_id, pr_url, pr_number in rows:
        # Derive pr_number from URL if not stored
        number = pr_number or pr_number_from_url(pr_url)
        if number is None:
            log.warning("poll_once: cannot determine PR number for %s — skipping", pr_url)
            continue

        # Skip dry-run placeholder URLs
        if "dry-run" in pr_url:
            log.debug("poll_once: skipping dry-run PR %s", pr_url)
            with conn.cursor() as cur:
                cur.execute(UPDATE_SQL, ("passing", now, False, None, row_id))
            conn.commit()
            updated += 1
            continue

        try:
            pr = get_pr(number)
        except Exception as exc:
            log.warning("poll_once: GitHub error for PR #%d: %s", number, exc)
            continue

        if pr is None:
            log.warning("poll_once: PR #%d not found — marking as closed", number)
            with conn.cursor() as cur:
                cur.execute(UPDATE_SQL, ("error", now, False, None, row_id))
            conn.commit()
            updated += 1
            continue

        merged: bool = pr.get("merged", False) or pr.get("merged_at") is not None
        merged_at: datetime | None = None
        if merged and pr.get("merged_at"):
            merged_at = datetime.fromisoformat(pr["merged_at"].replace("Z", "+00:00"))

        head_sha = pr.get("head", {}).get("sha", "")
        if head_sha:
            try:
                ci_status = get_check_runs(number, head_sha)
            except Exception as exc:
                log.warning("poll_once: check-runs error for PR #%d: %s", number, exc)
                ci_status = "error"
        else:
            ci_status = "error"

        log.info(
            "poll_once: PR #%d — merged=%s ci=%s",
            number, merged, ci_status,
        )

        with conn.cursor() as cur:
            cur.execute(UPDATE_SQL, (ci_status, now, merged, merged_at, row_id))
        conn.commit()
        updated += 1

        # Respect GitHub rate limit — 5 000 req/hr authenticated
        time.sleep(0.5)

    return updated


def main() -> None:
    if not GITHUB_TOKEN:
        log.error("GITHUB_TOKEN not set")
        sys.exit(1)
    if not DATABASE_URL:
        log.error("DATABASE_URL not set")
        sys.exit(1)

    log.info("pr_outcome_poller: connecting to database")
    try:
        conn = psycopg2.connect(DATABASE_URL)
    except Exception as exc:
        log.error("pr_outcome_poller: database connection failed: %s", exc)
        sys.exit(1)

    try:
        updated = poll_once(conn)
        log.info("pr_outcome_poller: done — %d rows updated", updated)
    finally:
        conn.close()


if __name__ == "__main__":
    main()
