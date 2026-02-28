from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any, Dict

from .backends.ibm import EhrenfestProgram, ehrenfest_to_ibm
from .optimize import optimize_qasm_with_stats


@dataclass
class CompileStats:
    backend: str
    gate_count_before: int
    gate_count_after: int


def _count_qasm_ops(qasm: str) -> int:
    count = 0
    for raw in qasm.splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            continue
        if line.startswith(("OPENQASM", "include", "qreg", "creg", "measure", "barrier")):
            continue
        count += 1
    return count


def compile_qasm(qasm: str, optimize: bool = False) -> Dict[str, Any]:
    """Compile raw OpenQASM and optionally run ZX optimization."""
    gate_count_before = _count_qasm_ops(qasm)
    if optimize:
        out_qasm, stats = optimize_qasm_with_stats(qasm)
        gate_count_after = stats["after"]
    else:
        out_qasm = qasm
        gate_count_after = gate_count_before
    return {
        "qasm": out_qasm,
        "stats": {
            "gate_count_before": gate_count_before,
            "gate_count_after": gate_count_after,
            "optimized": optimize,
        },
    }


def compile_for_backend(qasm: str, backend: str = "ibm_torino") -> Dict[str, Any]:
    """Compile OpenQASM with backend-aware transpilation path."""
    program = EhrenfestProgram(n_qubits=0, qasm=qasm, metadata={})
    transpiled = ehrenfest_to_ibm(program, backend_name=backend)

    before = _count_qasm_ops(qasm)
    depth_before = before
    try:
        after = int(transpiled.count_ops().total())  # qiskit OrderedDict with total()
    except Exception:
        # fallback when mocked/transpiled object does not expose count_ops
        after = before
    try:
        depth_after = int(transpiled.depth())
    except Exception:
        depth_after = after

    return {
        "backend": backend,
        "transpiled": transpiled,
        "stats": {
            **asdict(CompileStats(backend=backend, gate_count_before=before, gate_count_after=after)),
            "depth_before": depth_before,
            "depth_after": depth_after,
        },
    }
