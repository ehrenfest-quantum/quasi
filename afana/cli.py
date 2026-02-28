#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Iterable, Optional

from .backend_selector import (
    NoiseRequirements,
    ibm_backend_capabilities,
    select_backends,
    simulator_capabilities,
)
from .compile import compile_qasm


def _read_text(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def _print_gate_report(path: str, stats: dict) -> None:
    print(
        f"{path}: before={stats['gate_count_before']} "
        f"after={stats['gate_count_after']}"
    )


def _print_backend_selection(
    requirements: NoiseRequirements,
    backends: Iterable,
    selected_backend: Optional[str],
) -> None:
    print(
        "backend ranking: "
        f"t1>={requirements.t1_us_min:g}us "
        f"t2>={requirements.t2_us_min:g}us "
        f"qubits>={requirements.n_qubits_min}"
    )
    if requirements.gate_fidelity_min is not None:
        print(f"minimum gate fidelity: {requirements.gate_fidelity_min:.6f}")
    for backend in backends:
        marker = "*" if backend.name == selected_backend else "-"
        t1 = "inf" if backend.t1_us == float("inf") else f"{backend.t1_us:.1f}"
        t2 = "inf" if backend.t2_us == float("inf") else f"{backend.t2_us:.1f}"
        print(
            f"{marker} {backend.name}: t1={t1}us "
            f"t2={t2}us fidelity={backend.gate_fidelity:.6f} "
            f"qubits={backend.n_qubits}"
        )


def cmd_compile(
    path: str,
    optimize: bool,
    output: Optional[str],
    backend: Optional[str] = None,
) -> int:
    qasm = _read_text(path)
    result = compile_qasm(qasm, optimize=optimize)
    _print_gate_report(path, result["stats"])
    if backend:
        print(f"selected backend: {backend}")
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
    p_compile.add_argument(
        "--optimize",
        action="store_true",
        help="Enable ZX optimization",
    )
    p_compile.add_argument("--output", help="Optional output path for compiled QASM")
    p_compile.add_argument("--backend", help="Preferred IBM backend to inspect")
    p_compile.add_argument(
        "--select-backend",
        action="store_true",
        help="Rank available backends against noise requirements before compiling",
    )
    p_compile.add_argument(
        "--noise-t1-us",
        type=float,
        default=0.0,
        help="Minimum acceptable T1 coherence time in microseconds",
    )
    p_compile.add_argument(
        "--noise-t2-us",
        type=float,
        default=0.0,
        help="Minimum acceptable T2 coherence time in microseconds",
    )
    p_compile.add_argument(
        "--gate-fidelity-min",
        type=float,
        help="Minimum acceptable per-gate fidelity",
    )
    p_compile.add_argument(
        "--n-qubits",
        type=int,
        default=1,
        help="Minimum qubit capacity required by the program",
    )

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument(
        "--optimize",
        action="store_true",
        help="Enable ZX optimization",
    )

    args = parser.parse_args()
    if args.cmd == "compile":
        selected_backend = args.backend
        if args.select_backend:
            requirements = NoiseRequirements(
                t1_us_min=args.noise_t1_us,
                t2_us_min=args.noise_t2_us,
                n_qubits_min=args.n_qubits,
                gate_fidelity_min=args.gate_fidelity_min,
            )
            available = [simulator_capabilities(n_qubits=max(args.n_qubits, 32))]
            backend_name = selected_backend or "ibm_torino"
            try:
                available.append(ibm_backend_capabilities(backend_name))
            except Exception as exc:  # pragma: no cover - network/auth dependent
                print(
                    f"warning: could not load IBM backend '{backend_name}': {exc}",
                    file=sys.stderr,
                )
            ranked = select_backends(available, requirements)
            if not ranked:
                print(
                    "no backend satisfies the requested noise profile",
                    file=sys.stderr,
                )
                return 2
            if selected_backend is None:
                selected_backend = ranked[0].name
            _print_backend_selection(requirements, ranked, selected_backend)
        return cmd_compile(
            args.input,
            optimize=args.optimize,
            output=args.output,
            backend=selected_backend,
        )
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize)
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
