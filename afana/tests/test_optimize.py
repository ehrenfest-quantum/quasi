import sys
import types

from afana.optimize import optimize_qasm_with_stats


QASM_IN = "\n".join(
    [
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        "qreg q[2];",
        "h q[0];",
        "cx q[0],q[1];",
    ]
)


def _fake_pyzx(qasm_out: str):
    class _Optimized:
        def to_qasm(self):
            return qasm_out

    class _Circuit:
        @staticmethod
        def from_qasm(_src):
            class _Obj:
                @staticmethod
                def to_graph():
                    return object()

            return _Obj()

    fake = types.SimpleNamespace()
    fake.Circuit = _Circuit
    fake.simplify = types.SimpleNamespace(full_reduce=lambda _g: None)
    fake.extract_circuit = lambda _g: _Optimized()
    return fake


def test_optimize_never_worse_gate_count(monkeypatch):
    # Candidate has one extra gate, must fall back to original.
    qasm_worse = QASM_IN + "\nx q[0];\n"
    monkeypatch.setitem(sys.modules, "pyzx", _fake_pyzx(qasm_worse))
    out, stats = optimize_qasm_with_stats(QASM_IN)
    assert out == QASM_IN
    assert stats["before"] == 2
    assert stats["after"] == 2


def test_optimize_accepts_better_candidate(monkeypatch):
    qasm_better = "\n".join(
        [
            "OPENQASM 2.0;",
            'include "qelib1.inc";',
            "qreg q[2];",
            "h q[0];",
        ]
    )
    monkeypatch.setitem(sys.modules, "pyzx", _fake_pyzx(qasm_better))
    out, stats = optimize_qasm_with_stats(QASM_IN)
    assert out == qasm_better
    assert stats["before"] == 2
    assert stats["after"] == 1


def test_optimize_reduces_adjacent_t_gates(monkeypatch):
    qasm_t = "\n".join(
        [
            "OPENQASM 2.0;",
            'include "qelib1.inc";',
            "qreg q[1];",
            "t q[0];",
            "t q[0];",
            "h q[0];",
        ]
    )

    monkeypatch.setitem(sys.modules, "pyzx", _fake_pyzx(qasm_t))
    out, stats = optimize_qasm_with_stats(qasm_t)

    assert "s q[0];" in out
    assert "t q[0];\nt q[0];" not in out
    assert stats["before"] == 3
    assert stats["after"] == 2
    assert stats["t_before"] == 2
    assert stats["t_after"] == 0
