from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional


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
    if req.gate_fidelity_min is not None and backend.gate_fidelity < req.gate_fidelity_min:
        return False
    return True


def _score_backend(backend: BackendCapabilities, req: NoiseRequirements) -> float:
    # Prefer high fidelity and extra coherence headroom.
    t1_headroom = backend.t1_us - req.t1_us_min
    t2_headroom = backend.t2_us - req.t2_us_min
    qubit_headroom = backend.n_qubits - req.n_qubits_min
    return (backend.gate_fidelity * 1000.0) + t1_headroom + t2_headroom + (qubit_headroom * 0.1)


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
