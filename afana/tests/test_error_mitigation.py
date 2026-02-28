"""Integration tests for afana.backends.error_mitigation.

The tests verify both strategies without requiring real IBM hardware:
- MeasurementErrorMitigation: calibration matrix inversion and count correction
- RandomizedCompiling: Pauli-twirled circuit structure and idempotency
"""
from __future__ import annotations

from afana.backends.error_mitigation import (
    MeasurementErrorMitigation,
    RandomizedCompiling,
    MitigationStrategy,
    mitigate,
    available_strategies,
)


# ── Helpers ────────────────────────────────────────────────────────────────────

_BELL_QASM = "\n".join([
    "OPENQASM 2.0;",
    'include "qelib1.inc";',
    "qreg q[2];",
    "creg c[2];",
    "h q[0];",
    "cx q[0],q[1];",
    "measure q[0] -> c[0];",
    "measure q[1] -> c[1];",
])

# Perfect calibration (identity — no readout error)
_PERFECT_CAL = {0: [[1.0, 0.0], [0.0, 1.0]]}
# Typical readout error: 2% flip probability
_NOISY_CAL = {0: [[0.98, 0.02], [0.02, 0.98]]}


# ── MeasurementErrorMitigation ─────────────────────────────────────────────────

class TestMeasurementErrorMitigation:

    def test_is_mitigation_strategy(self):
        assert isinstance(MeasurementErrorMitigation(), MitigationStrategy)

    def test_description_nonempty(self):
        assert len(MeasurementErrorMitigation().description()) > 0

    def test_apply_adds_annotation(self):
        mem = MeasurementErrorMitigation(calibration=_NOISY_CAL)
        annotated = mem.apply(_BELL_QASM)
        assert "MeasurementErrorMitigation" in annotated
        assert _BELL_QASM in annotated

    def test_perfect_calibration_no_correction(self):
        """Identity calibration matrix leaves counts unchanged."""
        mem = MeasurementErrorMitigation(calibration=_PERFECT_CAL)
        counts = {"0": 500, "1": 500}
        corrected = mem.correct_counts(counts, n_qubits=1)
        assert abs(corrected["0"] - 500.0) < 1e-6
        assert abs(corrected["1"] - 500.0) < 1e-6

    def test_noisy_calibration_improves_fidelity(self):
        """With a biased readout, correction increases counts for |0> when it was over-counted."""
        # Calibration: 98% chance of reading correctly, 2% flip.
        # Simulate: prepared all |0>, but 2% are measured as |1|.
        cal = {0: [[0.98, 0.02], [0.02, 0.98]]}
        mem = MeasurementErrorMitigation(calibration=cal)
        raw_counts = {"0": 980, "1": 20}   # 1000 shots, 2% readout error
        corrected = mem.correct_counts(raw_counts, n_qubits=1)
        # After correction, |0> should be ≈ 1000 and |1> ≈ 0.
        assert corrected["0"] > 990, f"Expected >990 for |0>, got {corrected['0']}"
        assert corrected["1"] < 10, f"Expected <10 for |1>, got {corrected['1']}"

    def test_no_calibration_returns_unchanged_counts(self):
        """With no calibration data, correction is identity."""
        mem = MeasurementErrorMitigation()  # empty calibration
        counts = {"0": 600, "1": 400}
        corrected = mem.correct_counts(counts, n_qubits=1)
        assert abs(corrected["0"] - 600.0) < 1e-6
        assert abs(corrected["1"] - 400.0) < 1e-6

    def test_sum_of_corrected_counts_equals_total(self):
        """Corrected quasi-probabilities sum to the original total shot count."""
        cal = {0: [[0.95, 0.05], [0.05, 0.95]]}
        mem = MeasurementErrorMitigation(calibration=cal)
        counts = {"0": 700, "1": 300}
        corrected = mem.correct_counts(counts, n_qubits=1)
        total = sum(corrected.values())
        assert abs(total - 1000.0) < 1e-4


# ── RandomizedCompiling ────────────────────────────────────────────────────────

class TestRandomizedCompiling:

    def test_is_mitigation_strategy(self):
        assert isinstance(RandomizedCompiling(), MitigationStrategy)

    def test_description_nonempty(self):
        assert len(RandomizedCompiling().description()) > 0

    def test_apply_returns_string(self):
        rc = RandomizedCompiling(seed=42)
        result = rc.apply(_BELL_QASM)
        assert isinstance(result, str)
        assert len(result) > 0

    def test_twirled_circuit_preserves_cx(self):
        """The CNOT gate is present in the twirled output."""
        rc = RandomizedCompiling(seed=42)
        result = rc.apply(_BELL_QASM)
        assert "cx q[0],q[1];" in result

    def test_twirled_circuit_has_more_lines_than_original(self):
        """Twirling injects extra gates, so line count should be >= original."""
        rc = RandomizedCompiling(seed=42)
        result = rc.apply(_BELL_QASM)
        # May be equal if twirl selects identity; generally greater.
        assert len(result.splitlines()) >= len(_BELL_QASM.splitlines())

    def test_generate_twirled_circuits_count(self):
        """generate_twirled_circuits returns exactly n_samples variants."""
        rc = RandomizedCompiling(n_samples=5, seed=0)
        variants = rc.generate_twirled_circuits(_BELL_QASM)
        assert len(variants) == 5

    def test_generate_twirled_circuits_all_strings(self):
        rc = RandomizedCompiling(n_samples=3, seed=7)
        for v in rc.generate_twirled_circuits(_BELL_QASM):
            assert isinstance(v, str)
            assert "cx q[0],q[1];" in v

    def test_different_seeds_may_differ(self):
        """Two different seeds produce at least one different circuit."""
        rc_a = RandomizedCompiling(n_samples=10, seed=1)
        rc_b = RandomizedCompiling(n_samples=10, seed=2)
        variants_a = rc_a.generate_twirled_circuits(_BELL_QASM)
        variants_b = rc_b.generate_twirled_circuits(_BELL_QASM)
        # Very unlikely to be all the same
        assert variants_a != variants_b

    def test_twirled_circuits_contain_only_known_gates(self):
        """All gates in twirled output are from the standard Pauli + CNOT set."""
        rc = RandomizedCompiling(n_samples=20, seed=99)
        known = {"h", "cx", "cnot", "x", "y", "z", "measure", "qreg", "creg",
                 "OPENQASM", "include", "barrier", "//", ""}
        for variant in rc.generate_twirled_circuits(_BELL_QASM):
            for line in variant.splitlines():
                first = line.strip().split()[0] if line.strip() else ""
                assert first in known or first.startswith("//"), (
                    f"Unexpected gate in twirled circuit: {first!r}"
                )


# ── mitigate() wrapper ─────────────────────────────────────────────────────────

def test_mitigate_chains_strategies():
    """mitigate() applies strategies in order."""
    mem = MeasurementErrorMitigation(calibration=_NOISY_CAL)
    rc = RandomizedCompiling(seed=0)
    result = mitigate(_BELL_QASM, [mem, rc])
    assert "MeasurementErrorMitigation" in result
    assert "cx q[0],q[1];" in result


def test_mitigate_empty_strategies_passthrough():
    """mitigate() with an empty strategy list returns the original QASM."""
    assert mitigate(_BELL_QASM, []) == _BELL_QASM


# ── available_strategies() ─────────────────────────────────────────────────────

def test_available_strategies_has_two_entries():
    """Two strategies are registered (acceptance criterion)."""
    strategies = available_strategies()
    assert len(strategies) >= 2


def test_available_strategies_names():
    strategies = available_strategies()
    assert "measurement_error_mitigation" in strategies
    assert "randomized_compiling" in strategies


def test_available_strategies_instantiable():
    for _name, cls in available_strategies().items():
        obj = cls()
        assert isinstance(obj, MitigationStrategy)
