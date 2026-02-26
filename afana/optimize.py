from __future__ import annotations

from typing import Dict, Tuple


def _count_qasm_gates(qasm_str: str) -> int:
    count = 0
    for raw in qasm_str.splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            continue
        if line.startswith(("OPENQASM", "include", "qreg", "creg", "measure", "barrier")):
            continue
        count += 1
    return count


def optimize_qasm_with_stats(qasm_str: str) -> Tuple[str, Dict[str, int]]:
    """Run ZX-calculus simplification with a never-worse gate-count guarantee."""
    before = _count_qasm_gates(qasm_str)
    candidate = qasm_str

    try:
        import pyzx as zx  # type: ignore
    except Exception:
        return qasm_str, {"before": before, "after": before}

    try:
        circuit = zx.Circuit.from_qasm(qasm_str)
        graph = circuit.to_graph()
        zx.simplify.full_reduce(graph)
        optimized = zx.extract_circuit(graph)
        # Keep compatibility with varying pyzx API names.
        to_qasm = getattr(optimized, "to_qasm", None)
        if callable(to_qasm):
            candidate = to_qasm()
        to_qasm_v3 = getattr(optimized, "to_qasm3", None)
        if callable(to_qasm_v3) and candidate == qasm_str:
            candidate = to_qasm_v3()
    except Exception:
        return qasm_str, {"before": before, "after": before}

    after = _count_qasm_gates(candidate)
    if after > before:
        return qasm_str, {"before": before, "after": before}
    return candidate, {"before": before, "after": after}


def optimize_qasm(qasm_str: str) -> str:
    """Run optional ZX-calculus simplification for OpenQASM input."""
    optimized, _stats = optimize_qasm_with_stats(qasm_str)
    return optimized
