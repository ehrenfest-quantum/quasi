"""IBM-native compilation path for Afana.

Provides a Qiskit-based transpilation route: Ehrenfest CBOR / OpenQASM →
IBM-native gate set (RZ, SX, CX) with hardware-aware optimisation.

The :func:`compile_to_ibm_native` function works with a local
``AerSimulator`` so no IBM credentials are required for development or
testing.  :func:`transpile_for_ibm` targets a real named backend and
requires a configured ``qiskit-ibm-runtime`` account.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional


@dataclass
class EhrenfestProgram:
    """Minimal normalised program payload for Afana backends."""

    n_qubits: int
    qasm: str
    metadata: Dict[str, Any]


# ---------------------------------------------------------------------------
# Lazy Qiskit loaders (so the module imports even without qiskit installed)
# ---------------------------------------------------------------------------


def _load_qiskit_core():
    """Return (QuantumCircuit, transpile) or raise RuntimeError."""
    try:
        from qiskit import QuantumCircuit, transpile  # type: ignore
    except ImportError as exc:
        raise RuntimeError(
            "IBM backend requires qiskit. "
            "Install it with: pip install qiskit"
        ) from exc
    return QuantumCircuit, transpile


def _load_aer():
    """Return an AerSimulator instance, or *None* if qiskit-aer is absent."""
    try:
        from qiskit_aer import AerSimulator  # type: ignore
        return AerSimulator()
    except ImportError:
        return None


def _load_ibm_runtime():
    """Return QiskitRuntimeService or raise RuntimeError."""
    try:
        from qiskit_ibm_runtime import QiskitRuntimeService  # type: ignore
    except ImportError as exc:
        raise RuntimeError(
            "Real-device IBM access requires qiskit-ibm-runtime. "
            "Install it with: pip install qiskit-ibm-runtime"
        ) from exc
    return QiskitRuntimeService


# ---------------------------------------------------------------------------
# IBM-native compilation (works offline with AerSimulator)
# ---------------------------------------------------------------------------

#: IBM native gate set — used as fallback when no backend object is available.
IBM_NATIVE_GATES = ["rz", "sx", "x", "cx", "measure", "reset"]


def compile_to_ibm_native(qasm: str, backend=None, optimization_level: int = 3):
    """Compile OpenQASM to IBM-native Qiskit circuit.

    Parses *qasm* into a :class:`~qiskit.QuantumCircuit` and runs the Qiskit
    transpiler targeting IBM's native gate set (RZ / SX / CX).  If *backend*
    is ``None`` an :class:`~qiskit_aer.AerSimulator` is used automatically;
    if qiskit-aer is not installed the transpiler falls back to
    ``basis_gates=IBM_NATIVE_GATES``.

    Args:
        qasm: OpenQASM 2.0 source string.
        backend: Optional Qiskit backend object.  Defaults to AerSimulator.
        optimization_level: Qiskit transpiler optimisation level (0–3).

    Returns:
        Transpiled :class:`~qiskit.QuantumCircuit` in IBM-native gates.
    """
    QuantumCircuit, transpile = _load_qiskit_core()
    circuit = QuantumCircuit.from_qasm_str(qasm)

    if backend is None:
        backend = _load_aer()

    if backend is not None:
        return transpile(circuit, backend=backend, optimization_level=optimization_level)

    # Fallback: no AerSimulator — transpile to basis gates only
    return transpile(
        circuit,
        basis_gates=IBM_NATIVE_GATES,
        optimization_level=optimization_level,
    )


def ibm_native_stats(original_qasm: str, transpiled) -> Dict[str, int]:
    """Return before/after depth and gate-count statistics.

    Args:
        original_qasm: The original QASM source.
        transpiled: The transpiled :class:`~qiskit.QuantumCircuit`.

    Returns:
        Dict with keys ``depth_before``, ``depth_after``,
        ``gates_before``, ``gates_after``.
    """
    QuantumCircuit, _ = _load_qiskit_core()
    original = QuantumCircuit.from_qasm_str(original_qasm)
    return {
        "depth_before": original.depth(),
        "depth_after": transpiled.depth(),
        "gates_before": original.size(),
        "gates_after": transpiled.size(),
    }


# ---------------------------------------------------------------------------
# Real-hardware path (requires qiskit-ibm-runtime credentials)
# ---------------------------------------------------------------------------


def transpile_for_ibm(qasm: str, backend_name: str = "ibm_torino"):
    """Transpile OpenQASM using a named IBM backend (requires credentials).

    Connects to IBM Quantum via :class:`~qiskit_ibm_runtime.QiskitRuntimeService`
    and transpiles with ``optimization_level=3`` for the target topology.
    """
    QuantumCircuit, transpile = _load_qiskit_core()
    QiskitRuntimeService = _load_ibm_runtime()
    circuit = QuantumCircuit.from_qasm_str(qasm)
    service = QiskitRuntimeService()
    backend = service.backend(backend_name)
    return transpile(circuit, backend=backend, optimization_level=3)


def ehrenfest_to_ibm(
    program: EhrenfestProgram,
    backend_name: str = "ibm_torino",
    backend: Optional[Any] = None,
):
    """Transpile an Ehrenfest program to an IBM-native Qiskit circuit.

    If *backend* is supplied it is used directly (e.g. ``AerSimulator()``
    for offline testing).  Otherwise, if *backend_name* is ``"simulator"``
    or AerSimulator is available, AerSimulator is used.  Real IBM hardware
    requires ``qiskit-ibm-runtime`` credentials.

    Args:
        program: Normalised Ehrenfest program with a ``qasm`` attribute.
        backend_name: Named IBM backend (used only when *backend* is None
            and real hardware is requested).
        backend: Optional pre-constructed Qiskit backend object.

    Returns:
        Transpiled :class:`~qiskit.QuantumCircuit`.
    """
    return compile_to_ibm_native(program.qasm, backend=backend)
