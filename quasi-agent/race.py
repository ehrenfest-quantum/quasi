#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
race.py — Race orchestrator for the QUASI Pauli-Test.

Organises concurrent runs of solve.py across multiple models and tracks.
Each race is defined by a config (inline or --config file) that specifies:
  - which issues to solve
  - which participants compete (model IDs + their leaderboard track)
  - scoring rubric / jury reference

Tracks map to the four QUASI leaderboards:
  A  human         human-directed agents (gawain-openclaw, dkd-dobberkau …)
  B  oss           open-weights rotation (the 29 ROTATION models)
  C  commercial    proprietary APIs run separately (GPT-4o, Claude, Gemini)
  D  fleet         coordinated multi-agent ensemble

Usage:
    # List available race configs
    python3 quasi-agent/race.py --list

    # Dry-run the Grand Prix on issue #86 (show proposed edits, no commits)
    python3 quasi-agent/race.py --race grand-prix --issue 86 --dry-run

    # Run the OSS sprint on issue #86 (all rotation models, parallel)
    python3 quasi-agent/race.py --race oss-sprint --issue 86

    # Single-track race with specific models
    python3 quasi-agent/race.py --models deepseek-v3,mistral-small,apertus --issue 86

    # Show results of a saved race
    python3 quasi-agent/race.py --results /tmp/race-2026-02-24-grand-prix.json

Environment variables:
    GITHUB_TOKEN          required
    OPENROUTER_API_KEY    for openrouter models
    HF_TOKEN              for HuggingFace models
    SARVAM_API_KEY        for Sarvam
    QUASI_COMMERCIAL_KEY  for commercial track (separate, not in ROTATION)
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import concurrent.futures
from dataclasses import dataclass, field, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal

_here = Path(__file__).parent
sys.path.insert(0, str(_here))

from generate_issue import ROTATION, find_rotation_entry   # noqa: E402
from solve import build_context, call_model, apply_and_pr, gh  # noqa: E402

Track = Literal["A", "B", "C", "D"]


# ── Race configs ──────────────────────────────────────────────────────────────

@dataclass
class Participant:
    model_id: str
    track: Track
    # resolved at runtime
    model_entry: dict = field(default_factory=dict)


@dataclass
class RaceConfig:
    name: str
    description: str
    participants: list[Participant]
    max_workers: int = 8          # concurrent solve.py calls
    timeout_seconds: int = 300    # per-participant timeout
    jury: list[str] = field(default_factory=list)


@dataclass
class ParticipantResult:
    model_id: str
    track: Track
    status: Literal["success", "no_changes", "error", "timeout"]
    pr_url: str | None = None
    ledger_entry: int | None = None
    elapsed_seconds: float = 0.0
    error: str | None = None
    reasoning: str | None = None
    files_changed: list[str] = field(default_factory=list)


@dataclass
class RaceResult:
    race_name: str
    issue_number: int
    issue_title: str
    started_at: str
    finished_at: str
    participants: list[ParticipantResult]

    def winners_by_track(self) -> dict[str, list[ParticipantResult]]:
        tracks: dict[str, list[ParticipantResult]] = {}
        for p in self.participants:
            if p.status == "success":
                tracks.setdefault(p.track, []).append(p)
        # sort by elapsed time within each track
        for t in tracks:
            tracks[t].sort(key=lambda x: x.elapsed_seconds)
        return tracks


# ── Built-in race configs ──────────────────────────────────────────────────────

# Full OSS rotation — all 29 open-weights models on the same issue
OSS_SPRINT = RaceConfig(
    name="oss-sprint",
    description="All 29 open-weights rotation models race on one issue. Track B only.",
    participants=[Participant(model_id=e["id"], track="B") for e in ROTATION],
    max_workers=12,
    timeout_seconds=300,
    jury=["ehrenfest-spec", "pauli-test"],
)

# Curated Grand Prix — representative sample per geography + commercial track markers
GRAND_PRIX_PARTICIPANTS = [
    # Track B — OSS
    Participant("deepseek-v3",    "B"),   # China / MIT
    Participant("deepseek-r1",    "B"),   # China / MIT (reasoning)
    Participant("llama4",         "B"),   # US / Meta
    Participant("mistral-small",  "B"),   # France / Apache-2.0
    Participant("eurollm-22b",    "B"),   # EU consortium
    Participant("apertus",        "B"),   # Switzerland / ETH+EPFL
    Participant("qwq-32b",        "B"),   # China / reasoning
    Participant("phi-4",          "B"),   # US / Microsoft
    Participant("gemma-3-27b",    "B"),   # US / Google DeepMind
    Participant("command-a",      "B"),   # Canada / Cohere
    Participant("sarvam-m",       "B"),   # India
    Participant("swallow-70b",    "B"),   # Japan / Tokyo Tech
    Participant("sea-lion",       "B"),   # Singapore
    # Track C — commercial (config loaded from QUASI_COMMERCIAL_KEY, separate runner)
    # Participant("gpt-4o",         "C"),  # add when commercial runner ready
    # Participant("claude-opus",    "C"),
    # Participant("gemini-ultra",   "C"),
]

GRAND_PRIX = RaceConfig(
    name="grand-prix",
    description="Grand Prix: curated OSS fleet + (later) commercial track. Issue #86.",
    participants=GRAND_PRIX_PARTICIPANTS,
    max_workers=8,
    timeout_seconds=360,
    jury=["ehrenfest-spec", "pauli-test", "knuth-rubric", "lecun-rubric"],
)

# Fleet mode — all 29 models working as ensemble (their PRs visible to each other via main branch)
# Each wave sees merged PRs from previous wave — collaborative, not competitive.
FLEET_WAVE_SIZES = [5, 10, 14]   # wave 1 → 2 → 3, models per wave

FLEET = RaceConfig(
    name="fleet",
    description="50+ agent fleet: models run in waves, each wave builds on previous PRs.",
    participants=[Participant(model_id=e["id"], track="D") for e in ROTATION],
    max_workers=5,     # wave size
    timeout_seconds=400,
    jury=["ehrenfest-spec", "pauli-test"],
)

RACES: dict[str, RaceConfig] = {
    "oss-sprint": OSS_SPRINT,
    "grand-prix": GRAND_PRIX,
    "fleet": FLEET,
}


# ── Race runner ───────────────────────────────────────────────────────────────

def run_participant(
    participant: Participant,
    issue: dict,
    dry_run: bool,
) -> ParticipantResult:
    start = time.time()
    try:
        entry = find_rotation_entry(participant.model_id)
        context = build_context(issue)
        result = call_model(entry, context)
        reasoning = result.get("reasoning", "")
        changed = list({e["file"] for e in result.get("edits", [])}) + list(
            result.get("new_files", {}).keys()
        )
        if not changed:
            return ParticipantResult(
                model_id=participant.model_id,
                track=participant.track,
                status="no_changes",
                elapsed_seconds=time.time() - start,
                reasoning=reasoning,
            )
        pr_url = apply_and_pr(issue, entry, result, dry_run=dry_run)
        return ParticipantResult(
            model_id=participant.model_id,
            track=participant.track,
            status="success",
            pr_url=pr_url,
            elapsed_seconds=time.time() - start,
            reasoning=reasoning,
            files_changed=changed,
        )
    except Exception as e:
        return ParticipantResult(
            model_id=participant.model_id,
            track=participant.track,
            status="error",
            elapsed_seconds=time.time() - start,
            error=str(e)[:300],
        )


def run_race(
    config: RaceConfig,
    issue_number: int,
    dry_run: bool = False,
    output_file: Path | None = None,
) -> RaceResult:
    issue = gh("GET", f"/issues/{issue_number}")
    print(f"\n{'='*60}")
    print(f"  QUASI Pauli-Test — {config.name.upper()}")
    print(f"  Issue #{issue_number}: {issue['title']}")
    print(f"  Participants: {len(config.participants)}")
    print(f"  Workers: {config.max_workers}  Timeout: {config.timeout_seconds}s")
    if config.jury:
        print(f"  Jury: {', '.join(config.jury)}")
    print(f"  Mode: {'DRY RUN' if dry_run else 'LIVE'}")
    print(f"{'='*60}\n")

    started_at = datetime.now(timezone.utc).isoformat()
    results: list[ParticipantResult] = []

    with concurrent.futures.ThreadPoolExecutor(max_workers=config.max_workers) as ex:
        futures = {
            ex.submit(run_participant, p, issue, dry_run): p
            for p in config.participants
        }
        for future in concurrent.futures.as_completed(futures, timeout=config.timeout_seconds * 2):
            p = futures[future]
            try:
                r = future.result(timeout=config.timeout_seconds)
            except concurrent.futures.TimeoutError:
                r = ParticipantResult(
                    model_id=p.model_id, track=p.track,
                    status="timeout", elapsed_seconds=config.timeout_seconds,
                )
            results.append(r)
            _print_result(r)

    finished_at = datetime.now(timezone.utc).isoformat()
    race_result = RaceResult(
        race_name=config.name,
        issue_number=issue_number,
        issue_title=issue["title"],
        started_at=started_at,
        finished_at=finished_at,
        participants=results,
    )

    _print_summary(race_result)

    if output_file:
        output_file.write_text(json.dumps(asdict(race_result), indent=2))
        print(f"\nResults saved: {output_file}")

    return race_result


def _print_result(r: ParticipantResult) -> None:
    icons = {"success": "✓", "no_changes": "–", "error": "✗", "timeout": "⏱"}
    icon = icons.get(r.status, "?")
    elapsed = f"{r.elapsed_seconds:.1f}s"
    pr = f" → {r.pr_url}" if r.pr_url else ""
    err = f" ({r.error[:60]})" if r.error else ""
    print(f"  [{r.track}] {icon} {r.model_id:<25s} {elapsed:>6s}{pr}{err}")


def _print_summary(race: RaceResult) -> None:
    print(f"\n{'─'*60}")
    print(f"  RESULTS — {race.race_name} / Issue #{race.issue_number}")
    print(f"{'─'*60}")
    by_status: dict[str, int] = {}
    for p in race.participants:
        by_status[p.status] = by_status.get(p.status, 0) + 1
    for status, count in sorted(by_status.items()):
        print(f"  {status:<15s}: {count}")
    print()

    winners = race.winners_by_track()
    for track, prs in sorted(winners.items()):
        print(f"  Track {track} winner: {prs[0].model_id} ({prs[0].elapsed_seconds:.1f}s)")
    print(f"{'─'*60}\n")


# ── Fleet (wave) runner ────────────────────────────────────────────────────────

def run_fleet(issue_number: int, dry_run: bool = False) -> None:
    """
    Fleet mode: run models in waves. Each wave merges its PRs before the next
    wave starts — so later models see earlier contributions and build on them.
    """
    all_models = [p.model_id for p in FLEET.participants]
    waves = []
    idx = 0
    for size in FLEET_WAVE_SIZES:
        waves.append(all_models[idx:idx + size])
        idx += size
    if idx < len(all_models):
        waves.append(all_models[idx:])   # remainder

    issue = gh("GET", f"/issues/{issue_number}")
    print(f"\n{'='*60}")
    print(f"  QUASI Fleet — Issue #{issue_number}: {issue['title']}")
    print(f"  Total agents: {len(all_models)} in {len(waves)} waves")
    print(f"{'='*60}\n")

    for wave_idx, wave_models in enumerate(waves, 1):
        print(f"\n── Wave {wave_idx}/{len(waves)} ({len(wave_models)} agents) ──")
        wave_participants = [Participant(m, "D") for m in wave_models]
        wave_config = RaceConfig(
            name=f"fleet-wave-{wave_idx}",
            description="",
            participants=wave_participants,
            max_workers=FLEET_WAVE_SIZES[0],
        )
        run_race(wave_config, issue_number, dry_run=dry_run)
        if not dry_run and wave_idx < len(waves):
            print(f"Wave {wave_idx} complete. Waiting 10s before next wave…")
            time.sleep(10)


# ── CLI ────────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="QUASI Pauli-Test — Race Orchestrator"
    )
    parser.add_argument("--race", choices=list(RACES.keys()),
                        help="Named race config")
    parser.add_argument("--models",
                        help="Comma-separated model IDs for ad-hoc race")
    parser.add_argument("--issue", type=int,
                        help="GitHub issue number to solve")
    parser.add_argument("--dry-run", action="store_true",
                        help="Show proposed changes, no commits or PRs")
    parser.add_argument("--fleet", action="store_true",
                        help="Run in fleet/wave mode (Track D)")
    parser.add_argument("--output", type=Path,
                        help="Save race results JSON to this path")
    parser.add_argument("--results", type=Path,
                        help="Print summary of a saved race results JSON")
    parser.add_argument("--list", action="store_true",
                        help="List available race configs and exit")
    args = parser.parse_args()

    if args.list:
        print("\nAvailable races:\n")
        for name, config in RACES.items():
            print(f"  {name:<15s}  {config.description}")
            print(f"               {len(config.participants)} participants  "
                  f"jury: {', '.join(config.jury) or 'none'}")
        print()
        return

    if args.results:
        data = json.loads(args.results.read_text())
        race = RaceResult(**{k: v for k, v in data.items()
                             if k != "participants"})
        race.participants = [ParticipantResult(**p) for p in data["participants"]]
        _print_summary(race)
        return

    if not args.issue:
        parser.error("--issue is required")

    if not os.environ.get("GITHUB_TOKEN"):
        print("Error: GITHUB_TOKEN not set", file=sys.stderr)
        sys.exit(1)

    if args.fleet:
        run_fleet(args.issue, dry_run=args.dry_run)
        return

    if args.models:
        ids = [m.strip() for m in args.models.split(",")]
        config = RaceConfig(
            name="custom",
            description=f"Ad-hoc race: {', '.join(ids)}",
            participants=[Participant(m, "B") for m in ids],
        )
    elif args.race:
        config = RACES[args.race]
    else:
        parser.error("--race or --models required")

    output = args.output or Path(
        f"/tmp/race-{datetime.now().strftime('%Y%m%d-%H%M')}-{config.name}.json"
    )
    run_race(config, args.issue, dry_run=args.dry_run, output_file=output)


if __name__ == "__main__":
    main()
