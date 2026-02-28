from afana.ehrenfest import compile_ehrenfest_hex, emit_openqasm3, parse_param_bindings


PROGRAM = {
    "version": 2,
    "system": {"n_qubits": 2},
    "hamiltonian": {
        "terms": [
            {"coefficient": {"param": "theta_0"}, "paulis": [{"qubit": 0, "axis": 3}]},
            {"coefficient": {"param": "theta_1"}, "paulis": [{"qubit": 1, "axis": 1}]},
        ],
        "constant_offset": 0.0,
    },
    "evolution": {"total_us": 1.0, "steps": 1, "dt_us": 1.0},
    "observables": [{"type": "E"}],
    "noise": {"t1_us": 100.0, "t2_us": 80.0},
    "parameters": {"theta_0": 0.0, "theta_1": 0.0},
}


def test_emit_openqasm3_keeps_unbound_parameters():
    qasm = emit_openqasm3(PROGRAM)
    assert "OPENQASM 3.0;" in qasm
    assert "input float theta_0;" in qasm
    assert "input float theta_1;" in qasm
    assert "rz(theta_0) q[0];" in qasm
    assert "rx(theta_1) q[1];" in qasm


def test_emit_openqasm3_binds_parameters():
    qasm = emit_openqasm3(PROGRAM, bindings={"theta_0": 0.5, "theta_1": 1.2})
    assert "input float theta_0;" not in qasm
    assert "input float theta_1;" not in qasm
    assert "rz(0.5) q[0];" in qasm
    assert "rx(1.2) q[1];" in qasm


def test_parse_param_bindings_rejects_bad_shape():
    try:
        parse_param_bindings(["theta_0"])
    except ValueError:
        pass
    else:
        raise AssertionError("expected ValueError")


def test_compile_ehrenfest_hex_round_trip(tmp_path, monkeypatch):
    src = tmp_path / "vqe_h2.cbor.hex"
    src.write_text("a0\n", encoding="utf-8")

    monkeypatch.setattr("afana.ehrenfest.load_program", lambda path: PROGRAM)

    result = compile_ehrenfest_hex(str(src), bindings={"theta_0": 0.5})
    assert "input float theta_1;" in result["qasm"]
    assert "rz(0.5) q[0];" in result["qasm"]
    assert result["stats"]["gate_count_before"] == 2
