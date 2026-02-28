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


def ibm_circuit_from_ehrenfest(program: EhrenfestProgram):
    """Build a generic Qiskit circuit from an Ehrenfest program payload."""
    QuantumCircuit, _transpile, _service = _load_qiskit()
    return QuantumCircuit.from_qasm_str(program.qasm)


def ehrenfest_to_ibm(program: EhrenfestProgram, backend_name: str = "ibm_torino"):
    """Transpile an Ehrenfest program to an IBM-native Qiskit circuit."""
    QuantumCircuit, transpile, QiskitRuntimeService = _load_qiskit()
    circuit = QuantumCircuit.from_qasm_str(program.qasm)
    service = QiskitRuntimeService()
    backend = service.backend(backend_name)
    return transpile(circuit, backend=backend, optimization_level=3)


def _load_aer_simulator():
    try:
        from qiskit_aer import AerSimulator  # type: ignore
    except Exception as exc:  # pragma: no cover - exercised via tests
        raise RuntimeError(
            "IBM simulator path requires qiskit-aer. Install extras before using run_on_ibm_simulator."
        ) from exc
    return AerSimulator


def run_on_ibm_simulator(program: EhrenfestProgram, backend_name: str = "ibm_torino"):
    """Transpile an Ehrenfest program and run it on AerSimulator."""
    circuit = ehrenfest_to_ibm(program, backend_name=backend_name)
    AerSimulator = _load_aer_simulator()
    simulator = AerSimulator()
    result = simulator.run(circuit).result()
    return result.get_counts()
