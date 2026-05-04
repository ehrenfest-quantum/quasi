#!/usr/bin/env python3
"""Collect LOC stats per component and roadmap progress, write to Postgres."""

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

import psycopg2

# ---------------------------------------------------------------------------
# Component config
# ---------------------------------------------------------------------------

CORE_COMPONENTS = {"afana", "spec", "ts-halcontract", "urnery"}
INFRA_COMPONENTS = {
    "quasi-senate", "quasi-board", "quasi-agent", "quasi-mcp",
    "quasi-roster", "quasi-board-extensions", "benchmarks", "deploy", "infra",
}

LANG_MAP = {
    ".rs": "rust",
    ".py": "python",
    ".ts": "typescript",
    ".js": "javascript",
    ".cddl": "cddl",
    ".sql": "sql",
}

EXCLUDE_DIRS = {
    ".git", "target", "node_modules", "__pycache__", ".venv", "dist",
    ".mypy_cache", ".pytest_cache", ".ruff_cache",
}

EXCLUDE_FILES = {
    "package-lock.json", "yarn.lock", "Cargo.lock", "pnpm-lock.yaml",
}

# ---------------------------------------------------------------------------
# Roadmap — names + themes are human-curated; pct is computed from GitHub.
# ---------------------------------------------------------------------------

ROADMAP_META: list[tuple[int, str, str]] = [
    (1, "To All the Girls", "Ehrenfest v0.1 schema + examples"),
    (2, "Shake Your Rump", "Ehrenfest v0.2 operator algebra"),
    (3, "Johnny Ryall", "Ehrenfest parser (CBOR deserializer)"),
    (4, "Egg Man", "Afana compiler bootstrap"),
    (5, "High Plains Drifter", "ZX-IR intermediate representation"),
    (6, "The Sounds of Science", "ZX-calculus rewriting rules"),
    (7, "3-Minute Rule", "QASM3 codegen from ZX-IR"),
    (8, "Hey Ladies", "Qubit type checker"),
    (9, "5-Piece Chicken Dinner", "Hardware-aware compilation"),
    (10, "Looking Down the Barrel of a Gun", "Noise-aware compilation"),
    (11, "Car Thief", "Full ZX-calculus optimization"),
    (12, "What Comes Around", "Variational parameter support"),
    (13, "Shadrach", "Classical control flow"),
    (14, "Ask for Janice", "Ehrenfest v1.0 memory model"),
    (15, "B-Boy Bouillabaisse", "Quantum OS: Shor end-to-end"),
]

# Phases that the maintainer has explicitly declared complete. Closed-state
# of individual issues isn't sufficient because phases also include open
# follow-up issues; this set forces 100% regardless of issue counts.
MANUALLY_COMPLETED: frozenset[int] = frozenset({1})

GITHUB_REPO_DEFAULT = "ehrenfest-quantum/quasi"


def _gh_search_count(repo: str, token: str | None, query: str) -> int:
    """Return total_count for a GitHub Search-API issue query.

    Uses Search API because issue_search supports body matching and is
    paginated by total_count, which is exactly what we need for ratios.
    """
    url = "https://api.github.com/search/issues?per_page=1&q=" + urllib.parse.quote(
        f"repo:{repo} is:issue {query}"
    )
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, timeout=30) as resp:
        payload = json.load(resp)
    return int(payload.get("total_count", 0))


def compute_roadmap_progress(
    repo: str, token: str | None
) -> list[tuple[int, str, str, int, str, str]]:
    """Compute (phase, name, theme, pct, done, missing) per phase from issues.

    pct is closed / (closed + open) for issues whose body contains
    "PHASE-{NNN}". Phases in MANUALLY_COMPLETED report 100% regardless.
    Phases with no issues report 0%.
    """
    rows: list[tuple[int, str, str, int, str, str]] = []
    for phase, name, theme in ROADMAP_META:
        if phase in MANUALLY_COMPLETED:
            rows.append((phase, name, theme, 100, "phase declared complete", ""))
            continue

        token_str = f"PHASE-{phase:03d}"
        try:
            closed = _gh_search_count(repo, token, f'"{token_str}" in:body is:closed')
            open_ = _gh_search_count(repo, token, f'"{token_str}" in:body is:open')
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError) as exc:
            print(
                f"warning: GitHub query failed for {token_str}: {exc}; pct=0",
                file=sys.stderr,
            )
            closed, open_ = 0, 0

        total = closed + open_
        pct = round(closed / total * 100) if total > 0 else 0
        done = f"{closed}/{total} issues closed" if total else ""
        missing = f"{open_} open" if open_ else ""
        rows.append((phase, name, theme, pct, done, missing))
    return rows

# ---------------------------------------------------------------------------
# LOC counting
# ---------------------------------------------------------------------------


def count_lines(filepath):
    """Count non-empty lines in a file."""
    try:
        with open(filepath, "r", encoding="utf-8", errors="replace") as f:
            return sum(1 for line in f if line.strip())
    except (OSError, UnicodeDecodeError):
        return 0


def is_test_file(path):
    """Check if a file is a test file."""
    parts = path.parts
    name = path.name
    # In a tests/ directory
    if "tests" in parts or "test" in parts:
        return True
    # Python test files
    if name.startswith("test_") or name.endswith("_test.py"):
        return True
    # TypeScript/JS test files
    if name.endswith((".test.ts", ".test.js", ".spec.ts", ".spec.js")):
        return True
    # Rust test modules (handled differently — inline #[cfg(test)])
    return False


def scan_component(component_path):
    """Scan a component directory and return LOC stats per language."""
    stats = {}  # language -> {"source": int, "tests": int, "docs": int}

    for root, dirs, files in os.walk(component_path):
        # Prune excluded directories
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]

        root_path = Path(root)
        for fname in files:
            if fname in EXCLUDE_FILES:
                continue

            fpath = root_path / fname
            suffix = fpath.suffix.lower()

            # Docs
            if suffix == ".md":
                lang = "markdown"
                if lang not in stats:
                    stats[lang] = {"source": 0, "tests": 0, "docs": 0}
                stats[lang]["docs"] += count_lines(fpath)
                continue

            # Ehrenfest binary programs — count bytes, not lines
            if suffix == ".paul":
                lang = "ehrenfest"
                if lang not in stats:
                    stats[lang] = {"source": 0, "tests": 0, "docs": 0}
                try:
                    stats[lang]["source"] += fpath.stat().st_size
                except OSError:
                    pass
                continue

            # Source code
            lang = LANG_MAP.get(suffix)
            if lang is None:
                continue

            if lang not in stats:
                stats[lang] = {"source": 0, "tests": 0, "docs": 0}

            lines = count_lines(fpath)
            if is_test_file(fpath):
                stats[lang]["tests"] += lines
            else:
                stats[lang]["source"] += lines

    return stats


# ---------------------------------------------------------------------------
# DB operations
# ---------------------------------------------------------------------------


def write_to_db(db_url, repo_root, retention_days=30, github_repo=GITHUB_REPO_DEFAULT, github_token=None):
    """Scan repo components and write stats to Postgres."""
    conn = psycopg2.connect(db_url)
    conn.autocommit = False
    cur = conn.cursor()
    now = datetime.now(timezone.utc)

    # Clean old snapshots beyond retention
    cur.execute(
        "DELETE FROM project_stats WHERE snapshot_at < now() - interval '%s days'",
        (retention_days,),
    )
    cur.execute(
        "DELETE FROM roadmap_progress WHERE snapshot_at < now() - interval '%s days'",
        (retention_days,),
    )

    all_components = CORE_COMPONENTS | INFRA_COMPONENTS
    total_source = 0
    total_tests = 0
    total_docs = 0
    rows_inserted = 0

    print(f"{'Component':<28} {'Category':<15} {'Language':<12} {'Source':>8} {'Tests':>8} {'Docs':>8}")
    print("-" * 92)

    for comp_name in sorted(all_components):
        comp_path = repo_root / comp_name
        if not comp_path.is_dir():
            continue

        category = "core" if comp_name in CORE_COMPONENTS else "infrastructure"
        lang_stats = scan_component(comp_path)

        for lang, counts in sorted(lang_stats.items()):
            if lang == "markdown":
                # Docs go under the primary language or as separate row
                cur.execute(
                    """INSERT INTO project_stats
                       (snapshot_at, component, category, language, loc_source, loc_tests, loc_docs)
                       VALUES (%s, %s, %s, %s, %s, %s, %s)""",
                    (now, comp_name, category, "markdown", 0, 0, counts["docs"]),
                )
                total_docs += counts["docs"]
                rows_inserted += 1
                print(f"{comp_name:<28} {category:<15} {'markdown':<12} {0:>8} {0:>8} {counts['docs']:>8}")
                continue

            cur.execute(
                """INSERT INTO project_stats
                   (snapshot_at, component, category, language, loc_source, loc_tests, loc_docs)
                   VALUES (%s, %s, %s, %s, %s, %s, %s)""",
                (now, comp_name, category, lang, counts["source"], counts["tests"], counts["docs"]),
            )
            total_source += counts["source"]
            total_tests += counts["tests"]
            total_docs += counts["docs"]
            rows_inserted += 1
            print(f"{comp_name:<28} {category:<15} {lang:<12} {counts['source']:>8} {counts['tests']:>8} {counts['docs']:>8}")

    print("-" * 92)
    print(f"{'TOTAL':<28} {'':<15} {'':<12} {total_source:>8} {total_tests:>8} {total_docs:>8}")

    # Roadmap progress — computed live from GitHub issue counts per PHASE-NNN
    print(f"\n{'Phase':<5} {'Name':<30} {'%':>4}  {'Done':<25} {'Missing':<15}")
    print("-" * 90)
    roadmap_rows = compute_roadmap_progress(github_repo, github_token)
    for phase, name, theme, pct, done, missing in roadmap_rows:
        cur.execute(
            """INSERT INTO roadmap_progress
               (snapshot_at, phase, phase_name, theme, pct, done, missing)
               VALUES (%s, %s, %s, %s, %s, %s, %s)""",
            (now, phase, name, theme, pct, done, missing),
        )
        print(f"{phase:<5} {name:<30} {pct:>4}%  {done:<25} {missing:<15}")

    conn.commit()
    cur.close()
    conn.close()
    print(f"\nInserted {rows_inserted} project_stats rows + {len(roadmap_rows)} roadmap rows.")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(description="Collect QUASI project stats into Postgres")
    parser.add_argument(
        "--db-url",
        default=os.environ.get("DATABASE_URL"),
        help="Postgres connection URL (default: $DATABASE_URL)",
    )
    parser.add_argument(
        "--repo",
        default=str(Path(__file__).resolve().parents[2]),
        help="Path to quasi repo root (default: auto-detected)",
    )
    parser.add_argument(
        "--retention-days",
        type=int,
        default=30,
        help="Keep snapshots for N days (default: 30)",
    )
    parser.add_argument(
        "--github-repo",
        default=os.environ.get("GITHUB_REPO", GITHUB_REPO_DEFAULT),
        help=f"owner/name for roadmap issue queries (default: {GITHUB_REPO_DEFAULT})",
    )
    args = parser.parse_args()
    github_token = os.environ.get("GITHUB_TOKEN")

    if not args.db_url:
        print("Error: --db-url or DATABASE_URL required", file=sys.stderr)
        sys.exit(1)

    repo_root = Path(args.repo)
    if not (repo_root / "afana").is_dir():
        print(f"Error: {repo_root} does not look like the quasi repo root", file=sys.stderr)
        sys.exit(1)

    print(f"Scanning {repo_root} ...")
    write_to_db(
        args.db_url,
        repo_root,
        args.retention_days,
        github_repo=args.github_repo,
        github_token=github_token,
    )


if __name__ == "__main__":
    main()
