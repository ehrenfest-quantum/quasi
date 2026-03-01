from afana.backends.ibm import EhrenfestProgram, ehrenfest_to_ibm
from afana.compile import compile_for_backend


def test_ehrenfest_to_ibm_returns_hal_payload():
    """ehrenfest_to_ibm returns a HAL Contract SubmitCircuitInput dict."""
    qasm = "OPENQASM 2.0;\nqreg q[2];\nh q[0];"
    program = EhrenfestProgram(n_qubits=2, qasm=qasm, metadata={})
    result = ehrenfest_to_ibm(program, backend_name="ibm_torino")
    assert result["qasm"] == qasm
    assert result["backend"] == "ibm_torino"
    assert result["shots"] == 1024


def test_ehrenfest_to_ibm_respects_shots_metadata():
    program = EhrenfestProgram(
        n_qubits=1, qasm="OPENQASM 2.0;\nqreg q[1];", metadata={"shots": 512}
    )
    result = ehrenfest_to_ibm(program)
    assert result["shots"] == 512


def test_ehrenfest_to_ibm_backend_arg_ignored():
    """backend= kwarg is accepted for call-site compat but does not change output."""
    program = EhrenfestProgram(n_qubits=1, qasm="OPENQASM 2.0;\nqreg q[1];", metadata={})
    result = ehrenfest_to_ibm(program, backend_name="ibm_torino", backend=object())
    assert result["backend"] == "ibm_torino"


class _FakeTranspiled:
    def __init__(self, ops):
        self._ops = ops

    class _Ops:
        def __init__(self, n):
            self.n = n

        def total(self):
            return self.n

    def count_ops(self):
        return self._Ops(self._ops)


def test_compile_for_backend_returns_gate_stats(monkeypatch):
    def fake_ehrenfest_to_ibm(_program, backend_name):
        assert backend_name == "ibm_torino"
        return _FakeTranspiled(2)

    monkeypatch.setattr("afana.compile.ehrenfest_to_ibm", fake_ehrenfest_to_ibm)

    qasm = "\n".join([
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        "qreg q[2];",
        "h q[0];",
        "cx q[0],q[1];",
    ])
    out = compile_for_backend(qasm, backend="ibm_torino")
    assert out["stats"]["gate_count_before"] == 2
    assert out["stats"]["gate_count_after"] == 2
    assert out["backend"] == "ibm_torino"
