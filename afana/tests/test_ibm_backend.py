from afana.backends.ibm import EhrenfestProgram, ehrenfest_to_ibm
from afana.compile import compile_for_backend


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


def test_ehrenfest_to_ibm_uses_transpiler(monkeypatch):
    def fake_load():
        class QuantumCircuit:
            @staticmethod
            def from_qasm_str(_qasm):
                return object()

        def transpile(_circuit, backend, optimization_level):
            assert backend == "backend:ibm_torino"
            assert optimization_level == 3
            return _FakeTranspiled(3)

        class QiskitRuntimeService:
            def backend(self, name):
                return f"backend:{name}"

        return QuantumCircuit, transpile, QiskitRuntimeService

    monkeypatch.setattr("afana.backends.ibm._load_qiskit", fake_load)
    program = EhrenfestProgram(n_qubits=2, qasm="OPENQASM 2.0;\nqreg q[2];\nh q[0];", metadata={})
    out = ehrenfest_to_ibm(program, backend_name="ibm_torino")
    assert isinstance(out, _FakeTranspiled)


def test_compile_for_backend_returns_gate_stats(monkeypatch):
    def fake_ehrenfest_to_ibm(_program, backend_name):
        assert backend_name == "ibm_torino"
        return _FakeTranspiled(2)

    monkeypatch.setattr("afana.compile.ehrenfest_to_ibm", fake_ehrenfest_to_ibm)

    qasm = "\n".join(
        [
            "OPENQASM 2.0;",
            'include "qelib1.inc";',
            "qreg q[2];",
            "h q[0];",
            "cx q[0],q[1];",
        ]
    )
    out = compile_for_backend(qasm, backend="ibm_torino")
    assert out["stats"]["gate_count_before"] == 2
    assert out["stats"]["gate_count_after"] == 2
    assert out["backend"] == "ibm_torino"
