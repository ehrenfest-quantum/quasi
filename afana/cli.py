#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, Optional

from .compile import compile_for_backend, compile_qasm


def _read_text(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def _print_gate_report(path: str, stats: dict) -> None:
    line = (
        f"{path}: before={stats['gate_count_before']} "
        f"after={stats['gate_count_after']}"
    )
    depth_before = stats.get("depth_before")
    depth_after = stats.get("depth_after")
    if depth_before is not None and depth_after is not None:
        line += f" depth_before={depth_before} depth_after={depth_after}"
    print(line)


def cmd_compile(
    path: str,
    optimize: bool,
    output: Optional[str],
    backend: Optional[str] = None,
    emit: str = "qasm",
) -> int:
    qasm = _read_text(path)
    if backend or emit == "qiskit":
        chosen_backend = backend or "ibm_torino"
        result = compile_for_backend(qasm, backend=chosen_backend)
    else:
        result = compile_qasm(qasm, optimize=optimize)
    _print_gate_report(path, result["stats"])
    if output:
        payload = result.get("qasm", repr(result.get("transpiled")))
        Path(output).write_text(str(payload), encoding="utf-8")
    else:
        if emit == "qiskit":
            print(result["transpiled"])
        else:
            print(result["qasm"])
    return 0


def cmd_benchmark(paths: Iterable[str], optimize: bool, backend: Optional[str] = None) -> int:
    rows = []
    for path in paths:
        qasm = _read_text(path)
        if backend:
            result = compile_for_backend(qasm, backend=backend)
        else:
            result = compile_qasm(qasm, optimize=optimize)
        stats = result["stats"]
        rows.append(
            {
                "file": path,
                "before": stats["gate_count_before"],
                "after": stats["gate_count_after"],
            }
        )
        if "depth_before" in stats and "depth_after" in stats:
            rows[-1]["depth_before"] = stats["depth_before"]
            rows[-1]["depth_after"] = stats["depth_after"]
        _print_gate_report(path, stats)
    print(json.dumps({"results": rows}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Afana compiler CLI")
    sub = parser.add_subparsers(dest="cmd")

    p_compile = sub.add_parser("compile", help="Compile OpenQASM input")
    p_compile.add_argument("input", help="Path to OpenQASM file")
    p_compile.add_argument("--optimize", action="store_true", help="Enable ZX optimization")
    p_compile.add_argument("--backend", help="Optional backend target (e.g. ibm_torino)")
    p_compile.add_argument("--emit", choices=["qasm", "qiskit"], default="qasm",
                           help="Output format (default: %(default)s)")
    p_compile.add_argument("--output", help="Optional output path for compiled QASM")

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument("--optimize", action="store_true", help="Enable ZX optimization")
    p_bench.add_argument("--backend", help="Optional backend target (e.g. ibm_torino)")

    args = parser.parse_args()
    if args.cmd == "compile":
        return cmd_compile(
            args.input,
            optimize=args.optimize,
            output=args.output,
            backend=getattr(args, "backend", None),
            emit=getattr(args, "emit", "qasm"),
        )
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize, backend=getattr(args, "backend", None))
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
