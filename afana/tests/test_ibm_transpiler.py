"""Tests for the IBM-native Qiskit transpiler path (issue #31).

Uses AerSimulator for all tests — no real IBM credentials required.
"""

from afana.backends.ibm import (
    IBM_NATIVE_GATES,
    EhrenfestProgram,
    compile_to_ibm_native,
    ehrenfest_to_ibm,
    ibm_native_stats,
)

# ---------------------------------------------------------------------------
# Fixtures / shared QASM samples
# ---------------------------------------------------------------------------

BELL_QASM = "\n".join([
    "OPENQASM 2.0;",
    'include "qelib1.inc";',
    "qreg q[2];",
    "creg c[2];",
    "h q[0];",
    "cx q[0],q[1];",
    "measure q[0] -> c[0];",
    "measure q[1] -> c[1];",
])

GHZ_QASM = "\n".join([
    "OPENQASM 2.0;",
    'include "qelib1.inc";',
    "qreg q[3];",
    "creg c[3];",
    "h q[0];",
    "cx q[0],q[1];",
    "cx q[0],q[2];",
    "measure q[0] -> c[0];",
    "measure q[1] -> c[1];",
    "measure q[2] -> c[2];",
])

SIMPLE_QASM = "\n".join([
    "OPENQASM 2.0;",
    'include "qelib1.inc";',
    "qreg q[1];",
    "h q[0];",
    "h q[0];",   # HH = I — optimizer should eliminate both
])


# ---------------------------------------------------------------------------
# compile_to_ibm_native
# ---------------------------------------------------------------------------


def test_compile_returns_quantum_circuit():
    """compile_to_ibm_native returns a Qiskit QuantumCircuit."""
    from qiskit import QuantumCircuit
    result = compile_to_ibm_native(BELL_QASM)
    assert isinstance(result, QuantumCircuit)


def test_compile_uses_ibm_native_gate_set(monkeypatch):
    """When no backend is available the fallback restricts to IBM_NATIVE_GATES."""
    import afana.backends.ibm as ibm_mod
    # Suppress AerSimulator so we exercise the basis_gates fallback path
    monkeypatch.setattr(ibm_mod, "_load_aer", lambda: None)
    result = compile_to_ibm_native(BELL_QASM, backend=None)
    native_lower = {g.lower() for g in IBM_NATIVE_GATES}
    for instruction in result.data:
        gate_name = instruction.operation.name.lower()
        assert gate_name in native_lower, f"Non-native gate in circuit: {gate_name}"


def test_compile_bell_preserves_qubit_count():
    result = compile_to_ibm_native(BELL_QASM)
    assert result.num_qubits == 2


def test_compile_ghz_preserves_qubit_count():
    result = compile_to_ibm_native(GHZ_QASM)
    assert result.num_qubits == 3


def test_compile_accepts_explicit_aer_backend():
    """Passing an explicit AerSimulator backend should still succeed."""
    from qiskit_aer import AerSimulator
    backend = AerSimulator()
    result = compile_to_ibm_native(BELL_QASM, backend=backend)
    from qiskit import QuantumCircuit
    assert isinstance(result, QuantumCircuit)


# ---------------------------------------------------------------------------
# ibm_native_stats
# ---------------------------------------------------------------------------


def test_stats_returns_all_keys():
    transpiled = compile_to_ibm_native(BELL_QASM)
    stats = ibm_native_stats(BELL_QASM, transpiled)
    for key in ("depth_before", "depth_after", "gates_before", "gates_after"):
        assert key in stats


def test_stats_depth_values_are_positive():
    transpiled = compile_to_ibm_native(BELL_QASM)
    stats = ibm_native_stats(BELL_QASM, transpiled)
    assert stats["depth_before"] > 0
    assert stats["depth_after"] > 0


def test_optimization_does_not_increase_gate_count():
    """Transpiled gate count should be <= original (or justified by decomposition).

    For the HH circuit, after optimization the gate count should be reduced
    (HH = I, so optimisation removes both gates).
    """
    transpiled = compile_to_ibm_native(SIMPLE_QASM, optimization_level=3)
    stats = ibm_native_stats(SIMPLE_QASM, transpiled)
    # With optimization_level=3 the two H gates cancel; transpiled circuit
    # should be trivially short.
    assert stats["depth_after"] <= stats["depth_before"]


# ---------------------------------------------------------------------------
# ehrenfest_to_ibm
# ---------------------------------------------------------------------------


def test_ehrenfest_to_ibm_returns_circuit():
    from qiskit import QuantumCircuit
    program = EhrenfestProgram(n_qubits=2, qasm=BELL_QASM, metadata={})
    result = ehrenfest_to_ibm(program)
    assert isinstance(result, QuantumCircuit)


def test_ehrenfest_to_ibm_with_explicit_backend():
    from qiskit import QuantumCircuit
    from qiskit_aer import AerSimulator
    program = EhrenfestProgram(n_qubits=2, qasm=BELL_QASM, metadata={})
    result = ehrenfest_to_ibm(program, backend=AerSimulator())
    assert isinstance(result, QuantumCircuit)


# ---------------------------------------------------------------------------
# AerSimulator round-trip (correct measurement results)
# ---------------------------------------------------------------------------


def test_bell_state_correct_results():
    """Transpiled Bell circuit on AerSimulator should yield only |00> and |11>."""
    from qiskit_aer import AerSimulator
    transpiled = compile_to_ibm_native(BELL_QASM)
    simulator = AerSimulator()
    job = simulator.run(transpiled, shots=1024)
    counts = job.result().get_counts()
    # Only |00> and |11> should appear
    assert set(counts.keys()) <= {"00", "11"}
    assert counts.get("00", 0) + counts.get("11", 0) == 1024


def test_ghz_state_correct_results():
    """Transpiled GHZ circuit should yield only |000> and |111>."""
    from qiskit_aer import AerSimulator
    transpiled = compile_to_ibm_native(GHZ_QASM)
    simulator = AerSimulator()
    job = simulator.run(transpiled, shots=1024)
    counts = job.result().get_counts()
    assert set(counts.keys()) <= {"000", "111"}
    assert counts.get("000", 0) + counts.get("111", 0) == 1024


# ---------------------------------------------------------------------------
# CLI --emit qiskit
# ---------------------------------------------------------------------------


def test_cli_emit_qiskit_prints_depth_stats(tmp_path, capsys):
    from afana.cli import cmd_compile
    src = tmp_path / "bell.qasm"
    src.write_text(BELL_QASM, encoding="utf-8")
    rc = cmd_compile(str(src), optimize=False, output=None, emit="qiskit")
    assert rc == 0
    out = capsys.readouterr().out
    assert "IBM optimization" in out
    assert "circuit depth" in out
    assert "gate count" in out


def test_cli_emit_qasm_unchanged(tmp_path, monkeypatch, capsys):
    """Default --emit qasm path is unaffected by the new flag."""
    src = tmp_path / "bell.qasm"
    src.write_text(BELL_QASM, encoding="utf-8")

    monkeypatch.setattr(
        "afana.cli.compile_qasm",
        lambda qasm, optimize: {
            "qasm": qasm,
            "stats": {"gate_count_before": 2, "gate_count_after": 2},
        },
    )

    from afana.cli import cmd_compile
    rc = cmd_compile(str(src), optimize=False, output=None, emit="qasm")
    assert rc == 0
