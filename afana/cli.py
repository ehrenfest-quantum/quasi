#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict, Iterable, List, Optional

from .compile import compile_qasm
from .parametric import ParametricCompileError, compile_parametric


def _read_text(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def _print_gate_report(path: str, stats: dict) -> None:
    print(
        f"{path}: before={stats['gate_count_before']} "
        f"after={stats['gate_count_after']}"
    )


def _parse_bindings(param_args: List[str]) -> Dict[str, float]:
    """Parse ``--param name=value`` arguments into a dict."""
    bindings: Dict[str, float] = {}
    for arg in param_args or []:
        if "=" not in arg:
            raise ValueError(f"--param must be in the form name=value, got: {arg!r}")
        name, _, value = arg.partition("=")
        try:
            bindings[name.strip()] = float(value.strip())
        except ValueError:
            raise ValueError(f"--param value must be a float, got: {value!r}")
    return bindings


def cmd_compile(path: str, optimize: bool, output: Optional[str], param_args: Optional[List[str]] = None) -> int:
    qasm = _read_text(path)
    result = compile_qasm(qasm, optimize=optimize)
    _print_gate_report(path, result["stats"])
    if output:
        Path(output).write_text(result["qasm"], encoding="utf-8")
    else:
        print(result["qasm"])
    return 0


def cmd_compile_parametric(path: str, param_args: List[str], output: Optional[str]) -> int:
    """Compile an Ehrenfest program dict (JSON) with optional parameter bindings."""
    bindings = _parse_bindings(param_args)
    text = _read_text(path)
    try:
        program = json.loads(text)
    except json.JSONDecodeError as exc:
        print(f"Error: could not parse {path!r} as JSON: {exc}")
        return 1
    try:
        qasm3 = compile_parametric(program, bindings=bindings)
    except ParametricCompileError as exc:
        print(f"Parametric compile error: {exc}")
        return 1
    if bindings:
        print(f"Bound parameters: {bindings}")
    unbound = [
        t["coefficient"]["param"]
        for t in program.get("hamiltonian", {}).get("terms", [])
        if isinstance(t.get("coefficient"), dict) and t["coefficient"].get("param") not in bindings
    ]
    if unbound:
        print(f"Unbound parameters (emitted as 'input float[64]'): {unbound}")
    if output:
        Path(output).write_text(qasm3, encoding="utf-8")
    else:
        print(qasm3)
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

    p_parametric = sub.add_parser("compile-parametric", help="Compile Ehrenfest JSON program to QASM3")
    p_parametric.add_argument("input", help="Path to Ehrenfest program JSON file")
    p_parametric.add_argument(
        "--param", dest="params", action="append", default=[],
        metavar="NAME=VALUE",
        help="Bind a variational parameter to a concrete float value (repeatable)",
    )
    p_parametric.add_argument("--output", help="Optional output path for QASM3")

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument("--optimize", action="store_true", help="Enable ZX optimization")

    args = parser.parse_args()
    if args.cmd == "compile":
        return cmd_compile(args.input, optimize=args.optimize, output=args.output)
    if args.cmd == "compile-parametric":
        return cmd_compile_parametric(args.input, param_args=args.params, output=args.output)
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize)
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
