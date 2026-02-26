from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict


@dataclass
class EhrenfestProgram:
    """Minimal normalized program payload for Afana backends."""

    n_qubits: int
    qasm: str
    metadata: Dict[str, Any]


def _load_qiskit():
    try:
        from qiskit import QuantumCircuit, transpile  # type: ignore
        from qiskit_ibm_runtime import QiskitRuntimeService  # type: ignore
    except Exception as exc:  # pragma: no cover - exercised via tests
        raise RuntimeError(
            "IBM backend requires qiskit and qiskit-ibm-runtime. "
            "Install extras before using afana.backends.ibm."
        ) from exc
    return QuantumCircuit, transpile, QiskitRuntimeService


def transpile_for_ibm(qasm: str, backend_name: str = "ibm_torino"):
    """Transpile OpenQASM source using IBM backend-aware optimization."""
    QuantumCircuit, transpile, QiskitRuntimeService = _load_qiskit()
    circuit = QuantumCircuit.from_qasm_str(qasm)
    service = QiskitRuntimeService()
    backend = service.backend(backend_name)
    return transpile(circuit, backend=backend, optimization_level=3)


def ehrenfest_to_ibm(program: EhrenfestProgram, backend_name: str = "ibm_torino"):
    """Transpile an Ehrenfest program to an IBM-native Qiskit circuit."""
    return transpile_for_ibm(program.qasm, backend_name=backend_name)
