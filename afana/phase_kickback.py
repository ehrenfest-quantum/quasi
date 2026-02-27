from __future__ import annotations

from typing import Callable, Optional

from .circuit import Circuit

OracleBuilder = Callable[[Circuit, int], None]


def _to_qasm(circuit: Circuit) -> str:
    lines = ['OPENQASM 2.0;', 'include "qelib1.inc";', f"qreg q[{circuit.n_qubits}];"]
    for op in circuit.operations:
        if len(op.qubits) == 1:
            lines.append(f"{op.gate.lower()} q[{op.qubits[0]}];")
        elif len(op.qubits) == 2:
            lines.append(f"{op.gate.lower()} q[{op.qubits[0]}],q[{op.qubits[1]}];")
    return "\n".join(lines) + "\n"


def phase_kickback(target_qubit: int, oracle: OracleBuilder, n_qubits: int = 2) -> Circuit:
    """Build a named phase-kickback subroutine.

    Steps:
    1) Prepare ancilla in |-> (X then H on target qubit)
    2) Apply oracle to imprint phase on control space
    3) Optionally run ZX optimization pass on exported QASM
    """
    circuit = Circuit(n_qubits=n_qubits)
    circuit.add("X", [target_qubit], stage="prepare_ancilla")
    circuit.add("H", [target_qubit], stage="prepare_ancilla")
    oracle(circuit, target_qubit)

    try:
        from .optimize import optimize_qasm

        optimized_qasm = optimize_qasm(_to_qasm(circuit))
        circuit.add("ANNOTATE", [target_qubit], stage="zx", qasm_len=len(optimized_qasm))
    except Exception:
        pass

    return circuit
