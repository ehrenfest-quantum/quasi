#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict, Iterable, List, Optional

from .backend_selector import BackendCapabilities, NoiseRequirements, select_backends
from .compile import compile_qasm
from .optimize import reduce_t_gates
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


# ── Simulator backend (infinite coherence) ────────────────────────────────────

_SIMULATOR = BackendCapabilities(
    name="simulator",
    t1_us=1e9,
    t2_us=1e9,
    gate_fidelity=1.0,
    n_qubits=4096,
)


def cmd_compile(
    path: str, optimize: bool, output: Optional[str],
    reduce_t: bool = False, param_args: Optional[List[str]] = None,
) -> int:
    qasm = _read_text(path)
    if reduce_t:
        qasm, t_stats = reduce_t_gates(qasm)
        print(
            f"{path}: t_before={t_stats['t_before']} t_after={t_stats['t_after']} "
            f"(T-gate reduction saved {t_stats['t_before'] - t_stats['t_after']} gate(s))"
        )
    result = compile_qasm(qasm, optimize=optimize)
    _print_gate_report(path, result["stats"])
    if output:
        Path(output).write_text(result["qasm"], encoding="utf-8")
    else:
        print(result["qasm"])
    return 0


def cmd_compile_parametric(path: str, param_args: List[str], output: Optional[str]) -> int:
    """Compile an Ehrenfest .cbor.hex program with optional parameter bindings."""
    try:
        import cbor2
    except ImportError:
        print("Error: cbor2 required — pip install cbor2")
        return 1
    bindings = _parse_bindings(param_args)
    text = _read_text(path).strip()
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        print(f"Error: {path!r} is not valid hex: {exc}")
        return 1
    try:
        program = cbor2.loads(raw)
    except Exception as exc:
        print(f"Error: could not decode {path!r} as CBOR: {exc}")
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


def cmd_select_backend(
    t1_min: float,
    t2_min: float,
    n_qubits_min: int,
    fidelity_min: Optional[float],
    backends_json: Optional[str],
) -> int:
    """Print ranked backends that satisfy the given noise requirements."""
    backends = [_SIMULATOR]
    if backends_json:
        try:
            extra = json.loads(Path(backends_json).read_text(encoding="utf-8"))
            for b in extra:
                backends.append(BackendCapabilities(
                    name=b["name"],
                    t1_us=float(b["t1_us"]),
                    t2_us=float(b["t2_us"]),
                    gate_fidelity=float(b["gate_fidelity"]),
                    n_qubits=int(b["n_qubits"]),
                ))
        except Exception as exc:
            print(f"Error loading backends JSON: {exc}")
            return 1

    req = NoiseRequirements(
        t1_us_min=t1_min,
        t2_us_min=t2_min,
        n_qubits_min=n_qubits_min,
        gate_fidelity_min=fidelity_min,
    )

    print(f"Noise requirements: T1≥{t1_min}µs, T2≥{t2_min}µs, "
          f"n_qubits≥{n_qubits_min}"
          + (f", fidelity≥{fidelity_min}" if fidelity_min is not None else ""))
    print()

    try:
        ranked = select_backends(backends, req)
    except ValueError as exc:
        print(f"Invalid requirements: {exc}")
        return 1

    all_names = {b.name for b in backends}
    passing = {b.name for b in ranked}
    failing = all_names - passing

    for i, b in enumerate(ranked):
        marker = "[SELECTED]" if i == 0 else ""
        print(f"  ✓ {b.name}: T1={b.t1_us}µs, T2={b.t2_us}µs, "
              f"fidelity={b.gate_fidelity}, qubits={b.n_qubits}  {marker}")
    for name in sorted(failing):
        b_list = [b for b in backends if b.name == name]
        if b_list:
            b = b_list[0]
            reasons = []
            if b.t1_us < t1_min:
                reasons.append(f"T1={b.t1_us}µs < required {t1_min}µs")
            if b.t2_us < t2_min:
                reasons.append(f"T2={b.t2_us}µs < required {t2_min}µs")
            if b.n_qubits < n_qubits_min:
                reasons.append(f"n_qubits={b.n_qubits} < required {n_qubits_min}")
            if fidelity_min is not None and b.gate_fidelity < fidelity_min:
                reasons.append(f"fidelity={b.gate_fidelity} < required {fidelity_min}")
            print(f"  ✗ {b.name}: {'; '.join(reasons)}")

    if not ranked:
        print("No backend satisfies the requirements.")
        return 1
    return 0


def cmd_benchmark(paths: Iterable[str], optimize: bool, reduce_t: bool = False) -> int:
    rows = []
    for path in paths:
        qasm = _read_text(path)
        t_saved = 0
        if reduce_t:
            qasm, t_stats = reduce_t_gates(qasm)
            t_saved = t_stats["t_before"] - t_stats["t_after"]
        result = compile_qasm(qasm, optimize=optimize)
        stats = result["stats"]
        rows.append(
            {
                "file": path,
                "before": stats["gate_count_before"],
                "after": stats["gate_count_after"],
                "t_gates_saved": t_saved,
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
    p_compile.add_argument(
        "--reduce-t", action="store_true",
        help="Apply T-gate cancellation pass before ZX optimization",
    )
    p_compile.add_argument("--output", help="Optional output path for compiled QASM")

    p_parametric = sub.add_parser("compile-parametric", help="Compile Ehrenfest .cbor.hex program to QASM3")
    p_parametric.add_argument("input", help="Path to Ehrenfest program .cbor.hex file")
    p_parametric.add_argument(
        "--param", dest="params", action="append", default=[],
        metavar="NAME=VALUE",
        help="Bind a variational parameter to a concrete float value (repeatable)",
    )
    p_parametric.add_argument("--output", help="Optional output path for QASM3")

    p_backend = sub.add_parser("select-backend", help="Rank backends by noise requirements")
    p_backend.add_argument("--t1", type=float, default=50.0, metavar="T1_US",
                           help="Minimum required T1 relaxation time in µs (default: 50)")
    p_backend.add_argument("--t2", type=float, default=30.0, metavar="T2_US",
                           help="Minimum required T2 dephasing time in µs (default: 30)")
    p_backend.add_argument("--qubits", type=int, default=1, metavar="N",
                           help="Minimum number of qubits (default: 1)")
    p_backend.add_argument("--fidelity", type=float, default=None, metavar="F",
                           help="Minimum single-qubit gate fidelity [0,1] (optional)")
    p_backend.add_argument("--backends", metavar="FILE",
                           help="JSON file listing extra backend capabilities")

    p_bench = sub.add_parser("benchmark", help="Benchmark gate count before/after")
    p_bench.add_argument("inputs", nargs="+", help="One or more OpenQASM files")
    p_bench.add_argument("--optimize", action="store_true", help="Enable ZX optimization")
    p_bench.add_argument(
        "--reduce-t", action="store_true",
        help="Apply T-gate cancellation pass before ZX optimization",
    )

    args = parser.parse_args()
    if args.cmd == "compile":
        return cmd_compile(
            args.input, optimize=args.optimize,
            reduce_t=args.reduce_t, output=args.output,
        )
    if args.cmd == "compile-parametric":
        return cmd_compile_parametric(args.input, param_args=args.params, output=args.output)
    if args.cmd == "select-backend":
        return cmd_select_backend(
            t1_min=args.t1, t2_min=args.t2,
            n_qubits_min=args.qubits, fidelity_min=args.fidelity,
            backends_json=args.backends,
        )
    if args.cmd == "benchmark":
        return cmd_benchmark(args.inputs, optimize=args.optimize, reduce_t=args.reduce_t)
    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
