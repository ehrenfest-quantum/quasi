from __future__ import annotations

from dataclasses import dataclass
from math import inf
from typing import List, Optional


@dataclass(frozen=True)
class BackendCapabilities:
    name: str
    t1_us: float
    t2_us: float
    gate_fidelity: float
    n_qubits: int


@dataclass(frozen=True)
class NoiseRequirements:
    t1_us_min: float
    t2_us_min: float
    n_qubits_min: int
    gate_fidelity_min: Optional[float] = None


def _is_physically_valid(req: NoiseRequirements) -> bool:
    return req.t2_us_min <= 2.0 * req.t1_us_min


def _meets_requirements(
    backend: BackendCapabilities,
    req: NoiseRequirements,
) -> bool:
    if backend.t1_us < req.t1_us_min:
        return False
    if backend.t2_us < req.t2_us_min:
        return False
    if backend.n_qubits < req.n_qubits_min:
        return False
    if (
        req.gate_fidelity_min is not None
        and backend.gate_fidelity < req.gate_fidelity_min
    ):
        return False
    return True


def _score_backend(backend: BackendCapabilities, req: NoiseRequirements) -> float:
    # Prefer high fidelity and extra coherence headroom.
    t1_headroom = backend.t1_us - req.t1_us_min
    t2_headroom = backend.t2_us - req.t2_us_min
    qubit_headroom = backend.n_qubits - req.n_qubits_min
    return (
        (backend.gate_fidelity * 1000.0)
        + t1_headroom
        + t2_headroom
        + (qubit_headroom * 0.1)
    )


def select_backends(
    backends: List[BackendCapabilities],
    req: NoiseRequirements,
) -> List[BackendCapabilities]:
    """Return eligible backends sorted best-first."""
    if not _is_physically_valid(req):
        raise ValueError("Invalid noise requirements: must satisfy T2 <= 2*T1")

    eligible = [b for b in backends if _meets_requirements(b, req)]
    return sorted(eligible, key=lambda b: _score_backend(b, req), reverse=True)


def select_best_backend(
    backends: List[BackendCapabilities],
    req: NoiseRequirements,
) -> BackendCapabilities:
    ranked = select_backends(backends, req)
    if not ranked:
        raise RuntimeError("No backend satisfies the program noise requirements")
    return ranked[0]


def simulator_capabilities(n_qubits: int = 32) -> BackendCapabilities:
    """Return the static idealized simulator capability row."""
    return BackendCapabilities(
        name="simulator",
        t1_us=inf,
        t2_us=inf,
        gate_fidelity=1.0,
        n_qubits=n_qubits,
    )


def ibm_backend_capabilities(backend_name: str = "ibm_torino") -> BackendCapabilities:
    """Fetch IBM backend calibration data through Qiskit Runtime."""
    from .backends.ibm import _load_qiskit

    _QuantumCircuit, _transpile, QiskitRuntimeService = _load_qiskit()
    service = QiskitRuntimeService()
    backend = service.backend(backend_name)
    properties = backend.properties()

    t1_values = [
        float(item.t1)
        for item in getattr(properties, "qubits", [])
        if getattr(item, "t1", None) is not None
    ]
    t2_values = [
        float(item.t2)
        for item in getattr(properties, "qubits", [])
        if getattr(item, "t2", None) is not None
    ]

    fidelity_values: list[float] = []
    for gate in getattr(properties, "gates", []):
        for param in getattr(gate, "parameters", []):
            if getattr(param, "name", "") in {"gate_error", "error"}:
                fidelity_values.append(max(0.0, 1.0 - float(param.value)))

    t1_us = min(t1_values) / 1_000 if t1_values else 0.0
    t2_us = min(t2_values) / 1_000 if t2_values else 0.0
    gate_fidelity = min(fidelity_values) if fidelity_values else 0.0

    return BackendCapabilities(
        name=backend_name,
        t1_us=t1_us,
        t2_us=t2_us,
        gate_fidelity=gate_fidelity,
        n_qubits=int(getattr(backend, "num_qubits", 0)),
    )
