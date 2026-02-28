from afana.parser import parse
from afana.variational import compile_variational_file, emit_qasm3


def _source() -> str:
    return "\n".join(
        [
            'program "vqe"',
            "qubits 2",
            "vary theta_0 from 0.0 to 1.0 step 0.2",
            "rz theta_0 q0",
            "cx q0 q1",
            "endvary",
        ]
    )


def test_emit_qasm3_contains_parameter_update_logic():
    ast = parse(_source())
    qasm = emit_qasm3(ast, backend="ibm_torino")
    assert "OPENQASM 3.0;" in qasm
    assert "float theta_0 = 0.0;" in qasm
    assert "while (theta_0 <= 1.0) {" in qasm
    assert "rz(theta_0) q[0];" in qasm
    assert "theta_0 = theta_0 + 0.2;" in qasm


def test_compile_variational_file_emits_qasm3(tmp_path):
    src = tmp_path / "vqe.ef"
    src.write_text(_source(), encoding="utf-8")

    result = compile_variational_file(str(src), backend="ibm_torino")
    assert "OPENQASM 3.0;" in result["qasm"]
    assert "while (theta_0 <= 1.0) {" in result["qasm"]
    assert result["stats"]["gate_count_before"] == 3
