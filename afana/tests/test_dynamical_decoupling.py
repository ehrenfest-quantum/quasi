"""Tests for afana.backends.dynamical_decoupling."""

import pytest

from afana.backends.dynamical_decoupling import DD_SEQUENCES, apply_dynamical_decoupling


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_qasm(*gate_lines: str, n_qubits: int = 2) -> str:
    header = [
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        f"qreg q[{n_qubits}];",
    ]
    return "\n".join(header + list(gate_lines))


def _dd_lines(out: str) -> list:
    """Return lines that contain a DD annotation."""
    return [ln.strip() for ln in out.splitlines() if "// dd:" in ln]


def _non_dd_gate_lines(out: str) -> list:
    """Return original gate lines (no DD annotation, no header)."""
    return [
        ln.strip()
        for ln in out.splitlines()
        if ln.strip() and not ln.strip().startswith(("OPENQASM", "include", "qreg", "//"))
        and "// dd:" not in ln
    ]


# ---------------------------------------------------------------------------
# DD_SEQUENCES registry
# ---------------------------------------------------------------------------


def test_available_sequences():
    assert "cpmg" in DD_SEQUENCES
    assert "xy4" in DD_SEQUENCES
    assert "xy8" in DD_SEQUENCES


def test_cpmg_is_single_x():
    assert DD_SEQUENCES["cpmg"] == ["x"]


def test_xy4_is_four_pulses():
    assert DD_SEQUENCES["xy4"] == ["x", "y", "x", "y"]


def test_xy8_is_eight_pulses():
    assert DD_SEQUENCES["xy8"] == ["x", "y", "x", "y", "y", "x", "y", "x"]


# ---------------------------------------------------------------------------
# Basic insertion
# ---------------------------------------------------------------------------


def test_xy4_inserts_four_pulses():
    """q[1] idle while q[0] runs 3 gates → XY4 injected before cx."""
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
        "cx q[0],q[1];",
    )
    out = apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=2)
    dd = _dd_lines(out)
    # Should see x, y, x, y for q[1]
    assert len(dd) == 4
    assert all("q[1]" in ln for ln in dd)
    assert "x q[1]" in dd[0]
    assert "y q[1]" in dd[1]
    assert "x q[1]" in dd[2]
    assert "y q[1]" in dd[3]


def test_cpmg_inserts_single_x():
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
        "cx q[0],q[1];",
    )
    out = apply_dynamical_decoupling(qasm, sequence="cpmg", min_idle_layers=2)
    dd = _dd_lines(out)
    assert len(dd) == 1
    assert "x q[1]" in dd[0]


def test_xy8_inserts_eight_pulses():
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
        "cx q[0],q[1];",
    )
    out = apply_dynamical_decoupling(qasm, sequence="xy8", min_idle_layers=2)
    dd = _dd_lines(out)
    assert len(dd) == 8
    assert all("q[1]" in ln for ln in dd)


# ---------------------------------------------------------------------------
# Threshold enforcement
# ---------------------------------------------------------------------------


def test_no_dd_below_threshold():
    """Only 1 idle layer — threshold of 2 not reached — no DD inserted."""
    qasm = _make_qasm(
        "h q[0];",         # layer 0: q[1] idle (streak=1)
        "cx q[0],q[1];",   # layer 1: q[1] active — streak < 2, no DD
    )
    out = apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=2)
    assert _dd_lines(out) == []


def test_dd_inserted_at_exact_threshold():
    """Exactly min_idle_layers consecutive idle layers triggers insertion."""
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",          # q[1] idle for 2 layers
        "cx q[0],q[1];",
    )
    out = apply_dynamical_decoupling(qasm, sequence="cpmg", min_idle_layers=2)
    assert len(_dd_lines(out)) == 1


# ---------------------------------------------------------------------------
# Always-active qubit gets no DD
# ---------------------------------------------------------------------------


def test_no_dd_on_always_active_qubit():
    """A qubit that participates in every layer is never idle → no DD."""
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
    )
    # q[0] is active every layer; q[1] is idle but never reactivated
    # (trailing flush only if there's a reactivation or explicit trailing check)
    out = apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=2)
    # q[0] lines should not have DD annotations
    q0_dd = [ln for ln in _dd_lines(out) if "q[0]" in ln]
    assert q0_dd == []


# ---------------------------------------------------------------------------
# Trailing idle qubit
# ---------------------------------------------------------------------------


def test_dd_trailing_idle_qubit():
    """Qubit idle at end of circuit also receives DD (trailing flush)."""
    qasm = _make_qasm(
        "cx q[0],q[1];",
        "h q[0];",   # q[1] idle after cx
        "h q[0];",
        "h q[0];",   # 3 trailing idle layers for q[1]
    )
    out = apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=2)
    dd = _dd_lines(out)
    assert len(dd) == 4
    assert all("q[1]" in ln for ln in dd)


# ---------------------------------------------------------------------------
# Original gate ordering preserved
# ---------------------------------------------------------------------------


def test_original_gates_preserved():
    """All original gates appear in the output (DD only adds, never removes)."""
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
        "cx q[0],q[1];",
        "measure q[0] -> c[0];",
        n_qubits=2,
    )
    # Rebuild with creg
    qasm_full = qasm.replace("qreg q[2];", "qreg q[2];\ncreg c[2];")
    out = apply_dynamical_decoupling(qasm_full, sequence="cpmg", min_idle_layers=2)
    for gate in ("h q[0];", "cx q[0],q[1];"):
        assert gate in out


# ---------------------------------------------------------------------------
# Empty / trivial circuits
# ---------------------------------------------------------------------------


def test_no_gates_returns_unchanged():
    """Circuit with no gate lines returns the input unchanged."""
    qasm = "\n".join([
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        "qreg q[2];",
    ])
    assert apply_dynamical_decoupling(qasm, sequence="xy4") == qasm


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


def test_unknown_sequence_raises():
    qasm = _make_qasm("h q[0];")
    with pytest.raises(ValueError, match="Unknown DD sequence"):
        apply_dynamical_decoupling(qasm, sequence="bad_seq")


def test_min_idle_layers_zero_raises():
    qasm = _make_qasm("h q[0];")
    with pytest.raises(ValueError, match="min_idle_layers"):
        apply_dynamical_decoupling(qasm, sequence="xy4", min_idle_layers=0)


# ---------------------------------------------------------------------------
# Multi-qubit register
# ---------------------------------------------------------------------------


def test_dd_only_injected_for_idle_qubits():
    """In a 3-qubit circuit, always-active qubit gets no DD; idle one does."""
    # q[0] is active in every layer, q[2] is idle for the first 3 layers
    qasm = _make_qasm(
        "h q[0];",
        "h q[0];",
        "h q[0];",
        "cx q[0],q[2];",  # q[2] reactivated after 3 idle layers
        n_qubits=3,
    )
    out = apply_dynamical_decoupling(qasm, sequence="cpmg", min_idle_layers=2)
    dd = _dd_lines(out)
    # q[0] is always active — must never receive DD pulses
    q0_dd = [ln for ln in dd if "q[0]" in ln]
    assert q0_dd == []
    # q[2] was idle for 3 layers — must receive at least one DD pulse
    q2_dd = [ln for ln in dd if "q[2]" in ln]
    assert len(q2_dd) >= 1
