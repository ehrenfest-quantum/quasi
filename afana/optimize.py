from __future__ import annotations

import re
from typing import Dict, List, Tuple


# ── Gate-count helper ─────────────────────────────────────────────────────────

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


# ── T-gate reduction pass ─────────────────────────────────────────────────────

# Maps accumulated T-phase exponent (mod 8) to a minimal gate sequence.
# T has phase e^{iπ/4}, so T^n has phase e^{inπ/4}.
#   n=0 → identity (no gate)
#   n=1 → T
#   n=2 → S   (= T²)
#   n=3 → S·T (= T³)
#   n=4 → Z   (= T⁴)
#   n=5 → Z·T (= T⁵)
#   n=6 → Sdg (= T⁶)
#   n=7 → Tdg (= T⁷ = T⁻¹)
_PHASE_TO_GATES: Dict[int, List[str]] = {
    0: [],
    1: ["t"],
    2: ["s"],
    3: ["s", "t"],
    4: ["z"],
    5: ["z", "t"],
    6: ["sdg"],
    7: ["tdg"],
}

# Recognise a single-qubit T or Tdg gate line: "t q[0];" or "tdg q[0];"
_T_LINE_RE = re.compile(r"^(tdg|t)\s+(q\[\d+\])\s*;$", re.IGNORECASE)

# Extract all qubit references from a QASM line: "q[0]", "q[1]", …
_QUBIT_REF_RE = re.compile(r"q\[\d+\]")


def _emit_phase(qubit: str, phase: int) -> List[str]:
    """Return QASM lines for the reduced T-phase on *qubit*."""
    return [f"{g} {qubit};" for g in _PHASE_TO_GATES[phase % 8]]


def reduce_t_gates(qasm_str: str) -> Tuple[str, Dict[str, int]]:
    """Reduce adjacent T / Tdg gates on the same qubit to their minimal form.

    Uses the relation T^n = S (n=2), Z (n=4), Sdg (n=6), I (n=8), Tdg (n=7).
    T gates on *different* qubits are commuted past each other (they are all
    diagonal in the Z basis), allowing global cancellation within the run of
    consecutive single-qubit T/Tdg lines.

    Returns ``(optimised_qasm, stats)`` where *stats* contains:

    * ``t_before`` — total T / Tdg count in input
    * ``t_after``  — total T / Tdg count in output
    """

    def _count_t_gates(s: str) -> int:
        return sum(
            1 for ln in s.splitlines()
            if re.match(r"^\s*(tdg|t)\s+", ln.strip(), re.IGNORECASE)
        )

    t_before = _count_t_gates(qasm_str)

    # Per-qubit accumulated T-phase exponent (mod 8).
    qubit_phase: Dict[str, int] = {}
    output: List[str] = []

    def _flush_qubit(q: str) -> None:
        phase = qubit_phase.pop(q, 0)
        output.extend(_emit_phase(q, phase))

    def _flush_all() -> None:
        # Emit in a stable order (sorted by qubit index) for determinism.
        for q in sorted(qubit_phase):
            output.extend(_emit_phase(q, qubit_phase[q]))
        qubit_phase.clear()

    for raw_line in qasm_str.splitlines():
        line = raw_line.strip()

        m = _T_LINE_RE.match(line)
        if m:
            gate_name = m.group(1).lower()
            qubit = m.group(2)
            delta = 1 if gate_name == "t" else 7   # Tdg = T^7 mod 8
            qubit_phase[qubit] = (qubit_phase.get(qubit, 0) + delta) % 8
            continue

        # For any non-T line, flush the T-phase for every qubit it touches,
        # then emit the line.
        qubits_in_line = _QUBIT_REF_RE.findall(line)
        for q in dict.fromkeys(qubits_in_line):   # preserve first-seen order, deduplicate
            _flush_qubit(q)

        output.append(raw_line)

    # Flush any remaining T-phases at end of circuit.
    _flush_all()

    result = "\n".join(output)
    t_after = _count_t_gates(result)
    return result, {"t_before": t_before, "t_after": t_after}


# ── ZX-calculus optimisation ──────────────────────────────────────────────────

def optimize_qasm_with_stats(qasm_str: str, reduce_t: bool = False) -> Tuple[str, Dict[str, int]]:
    """Run ZX-calculus simplification with a never-worse gate-count guarantee.

    If *reduce_t* is ``True``, a T-gate cancellation pre-pass is applied first
    (see :func:`reduce_t_gates`).
    """
    if reduce_t:
        qasm_str, t_stats = reduce_t_gates(qasm_str)
    else:
        t_stats = {}

    before = _count_qasm_gates(qasm_str)
    candidate = qasm_str

    try:
        import pyzx as zx  # type: ignore
    except Exception:
        stats: Dict[str, int] = {"before": before, "after": before}
        stats.update(t_stats)
        return qasm_str, stats

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
        stats = {"before": before, "after": before}
        stats.update(t_stats)
        return qasm_str, stats

    after = _count_qasm_gates(candidate)
    if after > before:
        stats = {"before": before, "after": before}
        stats.update(t_stats)
        return qasm_str, stats
    stats = {"before": before, "after": after}
    stats.update(t_stats)
    return candidate, stats


def optimize_qasm(qasm_str: str, reduce_t: bool = False) -> str:
    """Run optional ZX-calculus simplification for OpenQASM input."""
    optimized, _stats = optimize_qasm_with_stats(qasm_str, reduce_t=reduce_t)
    return optimized
