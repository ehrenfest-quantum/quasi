import pytest

from afana.backend_selector import (
    BackendCapabilities,
    NoiseRequirements,
    ibm_backend_capabilities,
    select_backends,
    select_best_backend,
    simulator_capabilities,
)


def _sample_backends():
    return [
        BackendCapabilities(
            name="sim",
            t1_us=1000.0,
            t2_us=1500.0,
            gate_fidelity=0.999,
            n_qubits=32,
        ),
        BackendCapabilities(
            name="ibm_torino",
            t1_us=250.0,
            t2_us=300.0,
            gate_fidelity=0.987,
            n_qubits=133,
        ),
        BackendCapabilities(
            name="iqm_garnet",
            t1_us=180.0,
            t2_us=220.0,
            gate_fidelity=0.981,
            n_qubits=20,
        ),
    ]


def test_select_backends_filters_and_ranks():
    req = NoiseRequirements(
        t1_us_min=150.0,
        t2_us_min=200.0,
        n_qubits_min=16,
        gate_fidelity_min=0.98,
    )
    ranked = select_backends(_sample_backends(), req)
    assert [b.name for b in ranked] == ["sim", "ibm_torino", "iqm_garnet"]


def test_select_best_backend_rejects_when_no_backend_matches():
    req = NoiseRequirements(
        t1_us_min=5000.0,
        t2_us_min=6000.0,
        n_qubits_min=8,
        gate_fidelity_min=0.9999,
    )
    with pytest.raises(RuntimeError):
        select_best_backend(_sample_backends(), req)


def test_invalid_requirements_raise_value_error():
    # Violates T2 <= 2*T1 contract.
    req = NoiseRequirements(t1_us_min=10.0, t2_us_min=25.0, n_qubits_min=2)
    with pytest.raises(ValueError):
        select_backends(_sample_backends(), req)


def test_simulator_capabilities_always_passes():
    sim = simulator_capabilities(48)
    req = NoiseRequirements(
        t1_us_min=10_000.0,
        t2_us_min=20_000.0,
        n_qubits_min=48,
        gate_fidelity_min=0.999999,
    )

    assert sim.name == "simulator"
    assert sim.t1_us == float("inf")
    assert sim.t2_us == float("inf")
    assert sim.gate_fidelity == 1.0
    assert select_best_backend([sim], req) == sim


def test_ibm_backend_capabilities_reads_runtime_properties(monkeypatch):
    class _Param:
        def __init__(self, name, value):
            self.name = name
            self.value = value

    class _Gate:
        def __init__(self, parameters):
            self.parameters = parameters

    class _Qubit:
        def __init__(self, t1, t2):
            self.t1 = t1
            self.t2 = t2

    class _Props:
        qubits = [_Qubit(180_000.0, 260_000.0), _Qubit(200_000.0, 300_000.0)]
        gates = [
            _Gate([_Param("gate_error", 0.01)]),
            _Gate([_Param("error", 0.015)]),
        ]

    class _Backend:
        num_qubits = 127

        @staticmethod
        def properties():
            return _Props()

    class _Service:
        def backend(self, name):
            assert name == "ibm_sherbrooke"
            return _Backend()

    monkeypatch.setattr(
        "afana.backends.ibm._load_qiskit",
        lambda: (object, object, _Service),
    )

    caps = ibm_backend_capabilities("ibm_sherbrooke")
    assert caps.name == "ibm_sherbrooke"
    assert caps.t1_us == 180.0
    assert caps.t2_us == 260.0
    assert caps.gate_fidelity == 0.985
    assert caps.n_qubits == 127
