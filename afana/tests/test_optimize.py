import sys
import types

from afana.optimize import optimize_qasm_with_stats, reduce_t_gates


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


# ── T-gate reduction tests ────────────────────────────────────────────────────

def _qasm(gates: list[str], n_qubits: int = 1) -> str:
    header = [
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        f"qreg q[{n_qubits}];",
    ]
    return "\n".join(header + gates)


def test_reduce_t_pairs_to_s():
    """Two consecutive T gates on the same qubit reduce to S (acceptance criterion)."""
    qasm = _qasm(["t q[0];", "t q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 2
    assert stats["t_after"] == 0
    assert "s q[0];" in out
    assert "t q[0];" not in out


def test_reduce_t_and_tdg_cancel():
    """T followed by Tdg on the same qubit cancel to identity."""
    qasm = _qasm(["t q[0];", "tdg q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 2
    assert stats["t_after"] == 0
    assert "t q[0];" not in out
    assert "tdg q[0];" not in out


def test_reduce_tdg_pairs_to_sdg():
    """Two consecutive Tdg gates reduce to Sdg."""
    qasm = _qasm(["tdg q[0];", "tdg q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 2
    assert stats["t_after"] == 0
    assert "sdg q[0];" in out


def test_reduce_four_t_to_z():
    """Four T gates on the same qubit reduce to Z."""
    qasm = _qasm(["t q[0];", "t q[0];", "t q[0];", "t q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 4
    assert stats["t_after"] == 0
    assert "z q[0];" in out


def test_reduce_eight_t_to_identity():
    """Eight T gates cancel to identity (no gate emitted)."""
    qasm = _qasm(["t q[0];" for _ in range(8)])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 8
    assert stats["t_after"] == 0
    # No T, S, Z gates should remain for q[0]
    gate_lines = [ln for ln in out.splitlines() if "q[0]" in ln and "qreg" not in ln]
    assert gate_lines == []


def test_reduce_t_does_not_merge_across_h():
    """T gates separated by H on the same qubit must NOT be merged."""
    qasm = _qasm(["t q[0];", "h q[0];", "t q[0];"])
    out, stats = reduce_t_gates(qasm)
    # Both T gates remain, since H breaks the run.
    assert stats["t_before"] == 2
    assert stats["t_after"] == 2
    assert out.count("t q[0];") == 2


def test_reduce_t_different_qubits_merge():
    """T gates on different qubits are commuted and merged independently."""
    qasm = _qasm(["t q[0];", "t q[1];", "t q[0];"], n_qubits=2)
    out, stats = reduce_t_gates(qasm)
    # q[0]: T+T = S; q[1]: T remains
    assert stats["t_before"] == 3
    assert stats["t_after"] == 1   # only t q[1] remains
    assert "s q[0];" in out
    assert "t q[1];" in out
    assert "t q[0];" not in out


def test_reduce_t_single_t_unchanged():
    """A single T gate is not altered."""
    qasm = _qasm(["t q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 1
    assert stats["t_after"] == 1
    assert "t q[0];" in out


def test_reduce_t_no_t_gates():
    """Input without T gates is returned unchanged."""
    qasm = _qasm(["h q[0];", "cx q[0],q[0];"])
    out, stats = reduce_t_gates(qasm)
    assert stats["t_before"] == 0
    assert stats["t_after"] == 0
    assert out == qasm


def test_reduce_t_integrated_with_optimize(monkeypatch):
    """optimize_qasm_with_stats with reduce_t=True applies T reduction before ZX."""
    qasm = _qasm(["t q[0];", "t q[0];", "h q[0];"])
    monkeypatch.setitem(sys.modules, "pyzx", None)
    out, stats = optimize_qasm_with_stats(qasm, reduce_t=True)
    assert "s q[0];" in out
    assert stats["t_before"] == 2
    assert stats["t_after"] == 0
