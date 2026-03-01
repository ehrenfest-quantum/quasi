"""Tests for afana.parametric — parametric Ehrenfest program compiler."""
from __future__ import annotations

import pytest
from afana.parametric import (
    ParametricCompileError,
    bind_parameters,
    compile_parametric,
)


# ── Helpers ────────────────────────────────────────────────────────────────────

def _prog(terms, n_qubits=2, steps=1, total_us=1.0, params=None):
    """Minimal Ehrenfest program dict for testing."""
    dt_us = total_us / steps
    p = {
        "version": 2,
        "system": {"n_qubits": n_qubits},
        "hamiltonian": {"terms": terms, "constant_offset": 0.0},
        "evolution": {"total_us": total_us, "steps": steps, "dt_us": dt_us},
        "observables": [{"type": "SZ", "qubit": 0}],
        "noise": {"t1_us": 100.0, "t2_us": 80.0},
    }
    if params is not None:
        p["parameters"] = params
    return p


# ── compile_parametric ─────────────────────────────────────────────────────────

def test_compile_literal_z_term():
    """A literal Z term emits rz(angle) with concrete value."""
    prog = _prog([{"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 3}]}])
    qasm = compile_parametric(prog)
    assert "OPENQASM 3.0;" in qasm
    assert "rz(" in qasm
    assert "input float" not in qasm   # no params → no input declarations


def test_compile_parametric_z_term_emits_input_float():
    """A ParameterRef Z term emits input float[64] declaration."""
    prog = _prog(
        [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
        params={"theta": 0.5},
    )
    qasm = compile_parametric(prog)
    assert "input float[64] theta;" in qasm
    assert "rz(2.0*theta*" in qasm


def test_compile_parametric_x_term():
    """X rotation emits H rz H basis change."""
    prog = _prog(
        [{"coefficient": {"param": "alpha"}, "paulis": [{"qubit": 0, "axis": 1}]}],
        params={"alpha": 0.3},
    )
    qasm = compile_parametric(prog)
    assert "input float[64] alpha;" in qasm
    lines = [ln.strip() for ln in qasm.splitlines()]
    # X basis: H rz H pattern
    rz_idx = next(i for i, ln in enumerate(lines) if ln.startswith("rz("))
    assert lines[rz_idx - 1].startswith("h q[0]")
    assert lines[rz_idx + 1].startswith("h q[0]")


def test_compile_parametric_y_term():
    """Y rotation emits Sdg H rz H S basis change."""
    prog = _prog(
        [{"coefficient": {"param": "beta"}, "paulis": [{"qubit": 0, "axis": 2}]}],
        params={"beta": 0.7},
    )
    qasm = compile_parametric(prog)
    assert "input float[64] beta;" in qasm
    assert "sdg" in qasm
    assert "rz(2.0*beta*" in qasm


def test_compile_bound_parameter_becomes_literal():
    """When a parameter is bound, the emitted angle is a literal float."""
    prog = _prog(
        [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
        params={"theta": 0.5},
    )
    qasm = compile_parametric(prog, bindings={"theta": 0.5})
    # Bound → no input float declaration
    assert "input float" not in qasm
    # Literal angle appears (2 * 0.5 * dt_us = 1.0 * 1.0 = 1.0)
    assert "rz(1.0)" in qasm


def test_compile_two_qubit_zz_term():
    """ZZ two-qubit term emits CNOT-Rz-CNOT structure."""
    prog = _prog(
        [{"coefficient": 0.5, "paulis": [{"qubit": 0, "axis": 3}, {"qubit": 1, "axis": 3}]}],
        n_qubits=2,
    )
    qasm = compile_parametric(prog)
    assert "cx q[0], q[1];" in qasm
    assert "rz(" in qasm


def test_compile_multiple_params():
    """Multiple ParameterRef terms produce multiple input float declarations."""
    prog = _prog(
        [
            {"coefficient": {"param": "theta_0"}, "paulis": [{"qubit": 0, "axis": 3}]},
            {"coefficient": {"param": "theta_1"}, "paulis": [{"qubit": 1, "axis": 3}]},
        ],
        n_qubits=2,
        params={"theta_0": 0.5, "theta_1": 1.2},
    )
    qasm = compile_parametric(prog)
    assert "input float[64] theta_0;" in qasm
    assert "input float[64] theta_1;" in qasm


def test_compile_partial_binding():
    """Binding one of two params: only the unbound one gets input float."""
    prog = _prog(
        [
            {"coefficient": {"param": "theta_0"}, "paulis": [{"qubit": 0, "axis": 3}]},
            {"coefficient": {"param": "theta_1"}, "paulis": [{"qubit": 1, "axis": 3}]},
        ],
        n_qubits=2,
        params={"theta_0": 0.5, "theta_1": 1.2},
    )
    qasm = compile_parametric(prog, bindings={"theta_0": 0.5})
    assert "input float[64] theta_0;" not in qasm  # bound
    assert "input float[64] theta_1;" in qasm        # unbound


def test_compile_trotter_steps():
    """Programs with steps > 1 emit a for loop."""
    prog = _prog(
        [{"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 3}]}],
        steps=4, total_us=1.0,
    )
    qasm = compile_parametric(prog)
    assert "for uint i in [0:3]" in qasm


def test_compile_vqe_h2_example():
    """The canonical VQE H2 example round-trips through compile_parametric."""
    prog = {
        "version": 2,
        "system": {"n_qubits": 2},
        "hamiltonian": {
            "terms": [
                {"coefficient": {"param": "theta_0"}, "paulis": [{"qubit": 0, "axis": 3}]},
                {"coefficient": {"param": "theta_1"}, "paulis": [{"qubit": 1, "axis": 3}]},
                {"coefficient": 0.5, "paulis": [{"qubit": 0, "axis": 3}, {"qubit": 1, "axis": 3}]},
            ],
            "constant_offset": -1.137,
        },
        "evolution": {"total_us": 1.0, "steps": 4, "dt_us": 0.25},
        "observables": [{"type": "SZ", "qubit": 0}, {"type": "SZ", "qubit": 1}],
        "noise": {"t1_us": 100.0, "t2_us": 80.0, "gate_fidelity_min": 0.999},
        "parameters": {"theta_0": 0.5, "theta_1": 1.2},
    }
    qasm = compile_parametric(prog)
    assert "OPENQASM 3.0;" in qasm
    assert "input float[64] theta_0;" in qasm
    assert "input float[64] theta_1;" in qasm
    assert "cx q[0], q[1];" in qasm   # ZZ term


def test_compile_identity_term_skipped():
    """Identity Pauli term (axis=0) emits no gate (global phase)."""
    prog = _prog([{"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 0}]}])
    qasm = compile_parametric(prog)
    gate_lines = [ln for ln in qasm.splitlines()
                  if ln.strip() and not ln.startswith(("OPENQASM", "include", "//", "qubit", "bit", "c["))]
    assert gate_lines == [], f"Expected no gate lines for identity term, got: {gate_lines}"


# ── Error cases ───────────────────────────────────────────────────────────────

def test_compile_missing_system_raises():
    with pytest.raises(ParametricCompileError, match="system.n_qubits"):
        compile_parametric({"version": 2, "hamiltonian": {"terms": []}, "evolution": {}})


def test_compile_dt_mismatch_raises():
    prog = _prog([])
    prog["evolution"]["dt_us"] = 99.0  # wrong
    with pytest.raises(ParametricCompileError, match="dt_us"):
        compile_parametric(prog)


def test_compile_undeclared_param_raises():
    """ParameterRef not in parameters map and not in bindings raises error."""
    prog = _prog(
        [{"coefficient": {"param": "ghost"}, "paulis": [{"qubit": 0, "axis": 3}]}]
    )  # no params dict
    with pytest.raises(ParametricCompileError, match="ghost"):
        compile_parametric(prog)


# ── bind_parameters ───────────────────────────────────────────────────────────

def test_bind_parameters_replaces_refs():
    prog = _prog(
        [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
        params={"theta": 0.5},
    )
    bound = bind_parameters(prog, {"theta": 1.23})
    coeff = bound["hamiltonian"]["terms"][0]["coefficient"]
    assert isinstance(coeff, float)
    assert abs(coeff - 1.23) < 1e-9


def test_bind_parameters_missing_raises():
    prog = _prog(
        [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
        params={"theta": 0.5},
    )
    with pytest.raises(ParametricCompileError, match="theta"):
        bind_parameters(prog, {})


def test_bind_parameters_does_not_mutate_original():
    prog = _prog(
        [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
        params={"theta": 0.5},
    )
    bind_parameters(prog, {"theta": 1.0})
    # Original still has ParameterRef
    assert prog["hamiltonian"]["terms"][0]["coefficient"] == {"param": "theta"}


# ── CLI round-trip: CBOR → QASM3 ─────────────────────────────────────────────

def test_cli_compile_parametric_reads_cbor_hex(tmp_path):
    """compile-parametric CLI reads .cbor.hex, not JSON."""
    cbor2 = pytest.importorskip("cbor2")
    from afana.cli import cmd_compile_parametric

    prog = {
        "version": 2,
        "system": {"n_qubits": 1},
        "hamiltonian": {
            "terms": [{"coefficient": {"param": "theta"}, "paulis": [{"qubit": 0, "axis": 3}]}],
            "constant_offset": 0.0,
        },
        "evolution": {"total_us": 1.0, "steps": 1, "dt_us": 1.0},
        "observables": [{"type": "SZ", "qubit": 0}],
        "noise": {"t1_us": 100.0, "t2_us": 80.0},
        "parameters": {"theta": 0.5},
    }
    cbor_hex = cbor2.dumps(prog).hex()
    cbor_file = tmp_path / "test_prog.cbor.hex"
    cbor_file.write_text(cbor_hex)

    out_file = tmp_path / "out.qasm"
    rc = cmd_compile_parametric(str(cbor_file), [], str(out_file))
    assert rc == 0
    qasm = out_file.read_text()
    assert "OPENQASM 3.0;" in qasm
    assert "input float[64] theta;" in qasm


def test_cli_compile_parametric_rejects_json(tmp_path):
    """compile-parametric CLI rejects a plain JSON file (not valid hex)."""
    from afana.cli import cmd_compile_parametric
    json_file = tmp_path / "prog.json"
    json_file.write_text('{"version": 2}')
    rc = cmd_compile_parametric(str(json_file), [], None)
    assert rc == 1
