from __future__ import annotations

from typing import Dict, Tuple


def _is_t_gate(line: str) -> bool:
    return line.startswith("t ") and line.endswith(";")


def _target_qubit(line: str) -> str:
    return line[:-1].split(None, 1)[1].strip()


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


def _count_t_gates(qasm_str: str) -> int:
    count = 0
    for raw in qasm_str.splitlines():
        if _is_t_gate(raw.strip()):
            count += 1
    return count


def _reduce_t_gates(qasm_str: str) -> str:
    """Collapse adjacent T gates on the same qubit into the equivalent S gate."""
    reduced: list[str] = []
    pending_t: str | None = None

    for raw in qasm_str.splitlines():
        line = raw.strip()
        if _is_t_gate(line):
            if pending_t is not None and _target_qubit(pending_t) == _target_qubit(line):
                reduced.append(f"s {_target_qubit(line)};")
                pending_t = None
            else:
                if pending_t is not None:
                    reduced.append(pending_t)
                pending_t = line
            continue

        if pending_t is not None:
            reduced.append(pending_t)
            pending_t = None
        reduced.append(raw)

    if pending_t is not None:
        reduced.append(pending_t)

    return "\n".join(reduced)


def optimize_qasm_with_stats(qasm_str: str) -> Tuple[str, Dict[str, int]]:
    """Run ZX-calculus simplification with a never-worse gate-count guarantee."""
    before = _count_qasm_gates(qasm_str)
    t_before = _count_t_gates(qasm_str)
    baseline = _reduce_t_gates(qasm_str)
    candidate = baseline

    try:
        import pyzx as zx  # type: ignore
    except Exception:
        after = _count_qasm_gates(baseline)
        t_after = _count_t_gates(baseline)
        if after > before:
            return qasm_str, {"before": before, "after": before, "t_before": t_before, "t_after": t_before}
        return baseline, {"before": before, "after": after, "t_before": t_before, "t_after": t_after}

    try:
        circuit = zx.Circuit.from_qasm(baseline)
        graph = circuit.to_graph()
        zx.simplify.full_reduce(graph)
        optimized = zx.extract_circuit(graph)
        # Keep compatibility with varying pyzx API names.
        to_qasm = getattr(optimized, "to_qasm", None)
        if callable(to_qasm):
            candidate = _reduce_t_gates(to_qasm())
        to_qasm_v3 = getattr(optimized, "to_qasm3", None)
        if callable(to_qasm_v3) and candidate == baseline:
            candidate = _reduce_t_gates(to_qasm_v3())
    except Exception:
        after = _count_qasm_gates(baseline)
        t_after = _count_t_gates(baseline)
        if after > before:
            return qasm_str, {"before": before, "after": before, "t_before": t_before, "t_after": t_before}
        return baseline, {"before": before, "after": after, "t_before": t_before, "t_after": t_after}

    after = _count_qasm_gates(candidate)
    t_after = _count_t_gates(candidate)
    if after > before:
        return qasm_str, {"before": before, "after": before, "t_before": t_before, "t_after": t_before}
    return candidate, {"before": before, "after": after, "t_before": t_before, "t_after": t_after}


def optimize_qasm(qasm_str: str) -> str:
    """Run optional ZX-calculus simplification for OpenQASM input."""
    optimized, _stats = optimize_qasm_with_stats(qasm_str)
    return optimized
