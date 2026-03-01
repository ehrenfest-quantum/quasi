"""Dynamical decoupling pulse sequences for OpenQASM circuits.

Inserts XY4, CPMG, or XY8 pulse sequences into idle qubit windows to
counteract decoherence errors during execution on real hardware.  Works
directly on OpenQASM 2.0 strings without requiring Qiskit or hardware
access.

Typical usage::

    from afana.backends.dynamical_decoupling import apply_dynamical_decoupling

    qasm_with_dd = apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=2)
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Dict, List, Set

# ---------------------------------------------------------------------------
# Regex helpers
# ---------------------------------------------------------------------------

_QREG_RE = re.compile(r"^\s*qreg\s+(\w+)\[(\d+)\]\s*;")
_QUBIT_REF_RE = re.compile(r"\b(\w+)\[(\d+)\]")
_HEADER_RE = re.compile(r"^\s*(OPENQASM|include|gate|opaque|if|qreg|creg)\b")

# ---------------------------------------------------------------------------
# Supported DD sequences
# ---------------------------------------------------------------------------

#: Map of sequence name → ordered list of gate names for each pulse.
DD_SEQUENCES: Dict[str, List[str]] = {
    "cpmg": ["x"],
    "xy4": ["x", "y", "x", "y"],
    "xy8": ["x", "y", "x", "y", "y", "x", "y", "x"],
}


# ---------------------------------------------------------------------------
# Internal layer scheduler
# ---------------------------------------------------------------------------


@dataclass
class _Layer:
    """One parallel layer of gate lines."""

    gates: List[str] = field(default_factory=list)
    qubits: Set[str] = field(default_factory=set)


def _get_qubit_refs(line: str) -> Set[str]:
    """Return all qubit references (e.g. ``'q[0]'``) found in *line*."""
    return {f"{m.group(1)}[{m.group(2)}]" for m in _QUBIT_REF_RE.finditer(line)}


def _is_header_line(line: str) -> bool:
    stripped = line.strip()
    if not stripped or stripped.startswith("//"):
        return True
    return bool(_HEADER_RE.match(stripped))


def _schedule_layers(gate_lines: List[str]) -> List[_Layer]:
    """Greedily schedule *gate_lines* into parallel execution layers.

    Two gates land in the same layer only if their qubit sets are disjoint.
    Lines with no qubit references are placed in a singleton layer.
    """
    layers: List[_Layer] = []
    current = _Layer()
    for line in gate_lines:
        qrefs = _get_qubit_refs(line)
        if not qrefs:
            if current.gates:
                layers.append(current)
                current = _Layer()
            layers.append(_Layer(gates=[line], qubits=set()))
            continue
        if current.qubits & qrefs:
            layers.append(current)
            current = _Layer()
        current.gates.append(line)
        current.qubits |= qrefs
    if current.gates:
        layers.append(current)
    return layers


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def apply_dynamical_decoupling(
    qasm: str,
    sequence: str = "xy4",
    min_idle_layers: int = 2,
) -> str:
    """Insert dynamical decoupling pulses into idle qubit windows.

    The circuit is first partitioned into *parallel layers* (groups of gates
    with disjoint qubit sets).  Whenever a qubit has been idle for at least
    *min_idle_layers* consecutive layers and then becomes active again (or the
    circuit ends), a full DD sequence is injected just before the reactivating
    gate.  Each inserted line is annotated with ``// dd:<sequence>``.

    Args:
        qasm: OpenQASM 2.0 source string.
        sequence: DD sequence name — one of ``'cpmg'``, ``'xy4'``, ``'xy8'``.
        min_idle_layers: Minimum consecutive idle layers before DD is applied.
            Must be >= 1.

    Returns:
        Modified QASM string with DD pulses inserted.

    Raises:
        ValueError: If *sequence* is not recognised or *min_idle_layers* < 1.
    """
    if sequence not in DD_SEQUENCES:
        raise ValueError(
            f"Unknown DD sequence {sequence!r}. "
            f"Choose from {sorted(DD_SEQUENCES)}"
        )
    if min_idle_layers < 1:
        raise ValueError(f"min_idle_layers must be >= 1, got {min_idle_layers}")

    lines = qasm.splitlines()
    header: List[str] = []
    gate_lines: List[str] = []
    n_qubits_by_reg: Dict[str, int] = {}

    for line in lines:
        m = _QREG_RE.match(line)
        if m:
            n_qubits_by_reg[m.group(1)] = int(m.group(2))
        if not gate_lines and _is_header_line(line):
            header.append(line)
        else:
            gate_lines.append(line)

    all_qubits: List[str] = sorted(
        f"{reg}[{i}]"
        for reg, n in n_qubits_by_reg.items()
        for i in range(n)
    )

    if not gate_lines or not all_qubits:
        return qasm

    pulses = DD_SEQUENCES[sequence]
    layers = _schedule_layers(gate_lines)

    idle_streak: Dict[str, int] = {q: 0 for q in all_qubits}
    output_layers: List[List[str]] = []

    for layer in layers:
        active = layer.qubits
        dd_before: List[str] = []
        for qubit in all_qubits:
            if qubit in active:
                if idle_streak[qubit] >= min_idle_layers:
                    for pulse in pulses:
                        dd_before.append(f"{pulse} {qubit};  // dd:{sequence}")
                idle_streak[qubit] = 0
            else:
                idle_streak[qubit] += 1
        output_layers.append(dd_before + layer.gates)

    # Flush DD for qubits still idle at circuit end
    trailing: List[str] = []
    for qubit in all_qubits:
        if idle_streak[qubit] >= min_idle_layers:
            for pulse in pulses:
                trailing.append(f"{pulse} {qubit};  // dd:{sequence}")

    parts = list(header)
    for layer_lines in output_layers:
        parts.extend(layer_lines)
    parts.extend(trailing)

    return "\n".join(parts)
