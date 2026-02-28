#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, Optional

from .backends.ibm import compile_to_ibm_native, ibm_native_stats
from .compile import compile_qasm


def _read_text(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def _print_gate_report(path: str, stats: dict) -> None:
    print(
        f"{path}: before={stats['gate_count_before']} "
        f"after={stats['gate_count_after']}"
    )


def cmd_compile(path: str, optimize: bool, output: Optional[str], emit: str = "qasm") -> int:
    qasm = _read_text(path)

    if emit == "qiskit":
        try:
            transpiled = compile_to_ibm_native(qasm)
            stats = ibm_native_stats(qasm, transpiled)
        except RuntimeError as exc:
            print(f"Qiskit not available: {exc}")
            return 1
        print(
            f"{path}: circuit depth {stats['depth_before']} → {stats['depth_after']} "
            f"(IBM optimization), gate count {stats['gates_before']} → {stats['gates_after']}"
        )
        print(transpiled)
        return 0

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
        _print_gate_report(path, stats)
    print(json.dumps({"results": rows}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Afana compiler CLI")
    sub = parser.add_subparsers(dest="cmd")

    p_compile = sub.add_parser("compile", help="Compile OpenQASM input")
    p_compile.add_argument("input", help="Path to OpenQASM file")
    p_compile.add_argument("--optimize", action="store_true", help="Enable ZX optimization")
    p_compile.add_argument("--output", help="Optional output path for compiled QASM")
    p_compile.add_argument(
        "--emit", choices=["qasm", "qiskit"], default="qasm",
        help="Output format: 'qasm' (default) or 'qiskit' (IBM-native transpilation via Qiskit)",
    )

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument("--optimize", action="store_true", help="Enable ZX optimization")

    args = parser.parse_args()
    if args.cmd == "compile":
        return cmd_compile(args.input, optimize=args.optimize, output=args.output, emit=args.emit)
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize)
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
