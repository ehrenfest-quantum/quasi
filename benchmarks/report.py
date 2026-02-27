#!/usr/bin/env python3
"""Render benchmark JSON results as a markdown table."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

BASE_DIR = Path(__file__).parent
RESULTS_DIR = BASE_DIR / "results"


def _latest_results_file() -> Path:
    files = sorted(RESULTS_DIR.glob("*.json"))
    if not files:
        raise FileNotFoundError("No benchmark result files found in benchmarks/results/")
    return files[-1]


def render_markdown(results_file: Path) -> str:
    payload = json.loads(results_file.read_text(encoding="utf-8"))
    rows = payload.get("results", [])
    lines = [
        "| Circuit | Backend | Fidelity | Gate Count | Depth | Shots |",
        "|---|---|---:|---:|---:|---:|",
    ]
    for row in rows:
        lines.append(
            "| {circuit} | {backend} | {fidelity:.1f}% | {gate_count} | {depth} | {shots} |".format(
                circuit=row["circuit"],
                backend=row["backend"],
                fidelity=row["fidelity"] * 100.0,
                gate_count=row["gate_count"],
                depth=row["depth"],
                shots=row["shots"],
            )
        )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description="Render QUASI benchmark report")
    parser.add_argument("--input", help="Path to benchmark JSON (default: latest file in benchmarks/results)")
    args = parser.parse_args()

    src = Path(args.input) if args.input else _latest_results_file()
    print(render_markdown(src))


if __name__ == "__main__":
    main()
