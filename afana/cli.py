#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, Optional

from .compile import compile_qasm


def _read_text(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def _print_gate_report(path: str, stats: dict) -> None:
    line = (
        f"{path}: before={stats['gate_count_before']} "
        f"after={stats['gate_count_after']}"
    )
    if "t_gate_count_before" in stats and "t_gate_count_after" in stats:
        line += (
            f" t_before={stats['t_gate_count_before']}"
            f" t_after={stats['t_gate_count_after']}"
        )
    print(line)


def cmd_compile(path: str, optimize: bool, output: Optional[str]) -> int:
    qasm = _read_text(path)
    result = compile_qasm(qasm, optimize=optimize)
    _print_gate_report(path, result["stats"])
    if output:
        Path(output).write_text(result["qasm"], encoding="utf-8")
    else:
        print(result["qasm"])
    return 0


def cmd_benchmark(paths: Iterable[str], optimize: bool) -> int:
    rows = []
    for path in paths:
        qasm = _read_text(path)
        result = compile_qasm(qasm, optimize=optimize)
        stats = result["stats"]
        rows.append(
            {
                "file": path,
                "before": stats["gate_count_before"],
                "after": stats["gate_count_after"],
            }
        )
        if "t_gate_count_before" in stats and "t_gate_count_after" in stats:
            rows[-1]["t_before"] = stats["t_gate_count_before"]
            rows[-1]["t_after"] = stats["t_gate_count_after"]
        _print_gate_report(path, stats)
    print(json.dumps({"results": rows}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Afana compiler CLI")
    sub = parser.add_subparsers(dest="cmd")

    p_compile = sub.add_parser("compile", help="Compile OpenQASM input")
    p_compile.add_argument("input", help="Path to OpenQASM file")
    p_compile.add_argument("--optimize", action="store_true", help="Enable ZX optimization, including T-gate reduction")
    p_compile.add_argument("--output", help="Optional output path for compiled QASM")

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument("--optimize", action="store_true", help="Enable ZX optimization, including T-gate reduction")

    args = parser.parse_args()
    if args.cmd == "compile":
        return cmd_compile(args.input, optimize=args.optimize, output=args.output)
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize)
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
