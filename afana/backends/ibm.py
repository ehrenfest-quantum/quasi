"""IBM backend stub for Afana.

Afana's job is to emit standard OpenQASM.  Hardware-specific transpilation
(gate decomposition for the IBM Heron topology, noise-aware qubit mapping,
etc.) belongs in an IBM HAL driver that implements the HAL Contract API —
not in the compiler.

This module provides only the shared :class:`EhrenfestProgram` dataclass and
a thin passthrough that packages QASM for submission via the HAL Contract.
No vendor SDKs (Qiskit, qiskit-ibm-runtime, …) are imported here.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict


@dataclass
class EhrenfestProgram:
    """Minimal normalised program payload for Afana backends."""

    n_qubits: int
    qasm: str
    metadata: Dict[str, Any]


def ehrenfest_to_ibm(
    program: EhrenfestProgram,
    backend_name: str = "ibm_torino",
    backend: Any = None,
) -> Dict[str, Any]:
    """Package an Ehrenfest program for submission via the HAL Contract.

    Returns a plain dict ready to POST to ``/hal/jobs`` (see
    ``ts-halcontract`` / ``hal-contract.org``).  Hardware-native
    transpilation is performed by the IBM HAL driver on the server side.

    Args:
        program: Compiled Ehrenfest program carrying an OpenQASM string.
        backend_name: Target backend identifier (passed through to HAL).
        backend: Ignored — present only for call-site compatibility during
            the migration away from direct Qiskit usage.

    Returns:
        Dict with ``qasm``, ``backend``, and ``shots`` keys matching
        the HAL Contract ``SubmitCircuitInput`` schema.
    """
    return {
        "qasm": program.qasm,
        "backend": backend_name,
        "shots": program.metadata.get("shots", 1024),
    }
