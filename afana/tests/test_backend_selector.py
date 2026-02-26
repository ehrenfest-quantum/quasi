import pytest

from afana.backend_selector import (
    BackendCapabilities,
    NoiseRequirements,
    select_backends,
    select_best_backend,
)


def _sample_backends():
    return [
        BackendCapabilities(name="sim", t1_us=1000.0, t2_us=1500.0, gate_fidelity=0.999, n_qubits=32),
        BackendCapabilities(name="ibm_torino", t1_us=250.0, t2_us=300.0, gate_fidelity=0.987, n_qubits=133),
        BackendCapabilities(name="iqm_garnet", t1_us=180.0, t2_us=220.0, gate_fidelity=0.981, n_qubits=20),
    ]


def test_select_backends_filters_and_ranks():
    req = NoiseRequirements(t1_us_min=150.0, t2_us_min=200.0, n_qubits_min=16, gate_fidelity_min=0.98)
    ranked = select_backends(_sample_backends(), req)
    assert [b.name for b in ranked] == ["sim", "ibm_torino", "iqm_garnet"]


def test_select_best_backend_rejects_when_no_backend_matches():
    req = NoiseRequirements(t1_us_min=5000.0, t2_us_min=6000.0, n_qubits_min=8, gate_fidelity_min=0.9999)
    with pytest.raises(RuntimeError):
        select_best_backend(_sample_backends(), req)


def test_invalid_requirements_raise_value_error():
    # Violates T2 <= 2*T1 contract.
    req = NoiseRequirements(t1_us_min=10.0, t2_us_min=25.0, n_qubits_min=2)
    with pytest.raises(ValueError):
        select_backends(_sample_backends(), req)
