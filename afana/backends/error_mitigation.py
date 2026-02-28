"""Error mitigation strategies for IBM backend executions.

Two strategies are provided:

1. **MeasurementErrorMitigation** — applies a calibration-matrix correction to
   raw measurement counts, compensating for qubit readout errors.

2. **RandomizedCompiling** — performs Pauli twirling around CNOT gates, inserting
   random conjugate Pauli pairs before and after each two-qubit gate to convert
   coherent gate errors into stochastic Pauli noise (which is more tractable for
   error-correction decoders and reduces systematic bias in expectation values).

Both strategies operate on OpenQASM 2.0 source and/or raw shot-count dicts so
that they integrate cleanly with the Afana ZX-IR pipeline without requiring Qiskit
at import time.

No external dependencies — stdlib only.
"""
from __future__ import annotations

import random
import re
from abc import ABC, abstractmethod
from typing import Any, Dict, List, Optional, Sequence, Tuple


# ── Base ──────────────────────────────────────────────────────────────────────

class MitigationStrategy(ABC):
    """Abstract base for error mitigation strategies."""

    @abstractmethod
    def apply(self, qasm: str) -> str:
        """Return a modified (or instrumented) QASM string."""

    @abstractmethod
    def description(self) -> str:
        """Human-readable description of the strategy."""


# ── Strategy 1: Measurement Error Mitigation ─────────────────────────────────

class MeasurementErrorMitigation(MitigationStrategy):
    """Correct raw measurement counts using a per-qubit calibration matrix.

    The calibration matrix ``A`` is a 2×2 matrix where ``A[i, j]`` is the
    empirical probability of measuring outcome *i* when qubit was prepared in
    state *j*::

        A = [[P(0|0), P(0|1)],
             [P(1|0), P(1|1)]]

    The corrected quasi-probability vector is obtained by applying the
    pseudo-inverse of the tensor-product calibration matrix to the raw counts.

    For multi-qubit systems the per-qubit matrices are tensor-producted.

    Parameters
    ----------
    calibration:
        Optional dict mapping qubit index → 2×2 calibration matrix
        (as a list-of-lists).  When ``None`` an identity calibration is used
        (no correction applied).
    """

    def __init__(self, calibration: Optional[Dict[int, List[List[float]]]] = None) -> None:
        self.calibration = calibration or {}

    # ── Classical post-processing ────────────────────────────────────────────

    @staticmethod
    def _invert_2x2(m: List[List[float]]) -> List[List[float]]:
        """Return the inverse of a 2×2 matrix, raising ValueError if singular."""
        det = m[0][0] * m[1][1] - m[0][1] * m[1][0]
        if abs(det) < 1e-12:
            raise ValueError("Calibration matrix is singular — cannot invert")
        inv_det = 1.0 / det
        return [
            [m[1][1] * inv_det, -m[0][1] * inv_det],
            [-m[1][0] * inv_det, m[0][0] * inv_det],
        ]

    @staticmethod
    def _tensor_product(a: List[List[float]], b: List[List[float]]) -> List[List[float]]:
        """Compute the Kronecker (tensor) product of two matrices."""
        ra, ca = len(a), len(a[0])
        rb, cb = len(b), len(b[0])
        result: List[List[float]] = [
            [0.0] * (ca * cb) for _ in range(ra * rb)
        ]
        for i in range(ra):
            for j in range(ca):
                for k in range(rb):
                    for ll in range(cb):
                        result[i * rb + k][j * cb + ll] = a[i][j] * b[k][ll]
        return result

    @staticmethod
    def _matvec(m: List[List[float]], v: List[float]) -> List[float]:
        """Multiply matrix *m* by column vector *v*."""
        return [sum(m[i][j] * v[j] for j in range(len(v))) for i in range(len(m))]

    def correct_counts(self, counts: Dict[str, int], n_qubits: int) -> Dict[str, float]:
        """Return corrected quasi-probability counts.

        Parameters
        ----------
        counts:
            Raw shot counts, e.g. ``{"00": 480, "11": 520}``.
        n_qubits:
            Number of measured qubits (determines bitstring width).

        Returns
        -------
        dict
            Corrected quasi-probabilities (may be negative due to inversion).
            Sum is normalised to match the total shot count.
        """
        dim = 2 ** n_qubits
        total = sum(counts.values()) or 1

        # Build the full calibration matrix (tensor product of per-qubit matrices).
        identity = [[1.0, 0.0], [0.0, 1.0]]
        cal_matrix: List[List[float]] = [[1.0]]  # 1×1 identity to start
        for q in range(n_qubits):
            per_qubit = self.calibration.get(q, identity)
            cal_matrix = self._tensor_product(cal_matrix, per_qubit)

        inv_cal = self._invert_2x2(cal_matrix) if n_qubits == 1 else self._invert_nxn(cal_matrix)

        # Convert counts to a probability vector (little-endian bitstring ordering).
        raw_vec = [0.0] * dim
        for bitstring, cnt in counts.items():
            idx = int(bitstring, 2)
            raw_vec[idx] = cnt / total

        corrected_vec = self._matvec(inv_cal, raw_vec)

        # Convert back to counts-like dict.
        return {
            format(i, f"0{n_qubits}b"): corrected_vec[i] * total
            for i in range(dim)
        }

    @staticmethod
    def _invert_nxn(m: List[List[float]]) -> List[List[float]]:
        """Invert an n×n matrix via Gaussian elimination with partial pivoting."""
        n = len(m)
        aug = [[m[i][j] for j in range(n)] + [1.0 if i == j else 0.0 for j in range(n)]
               for i in range(n)]
        for col in range(n):
            # Partial pivot
            pivot = max(range(col, n), key=lambda r: abs(aug[r][col]))
            aug[col], aug[pivot] = aug[pivot], aug[col]
            if abs(aug[col][col]) < 1e-12:
                raise ValueError("Calibration matrix is singular — cannot invert")
            scale = aug[col][col]
            aug[col] = [x / scale for x in aug[col]]
            for row in range(n):
                if row != col:
                    factor = aug[row][col]
                    aug[row] = [aug[row][j] - factor * aug[col][j] for j in range(2 * n)]
        return [[aug[i][n + j] for j in range(n)] for i in range(n)]

    # ── QASM instrumentation ─────────────────────────────────────────────────

    def apply(self, qasm: str) -> str:
        """Return QASM with calibration-annotation comments injected.

        In a live execution workflow the QASM is unchanged; post-processing
        (``correct_counts``) is applied to the result histogram instead.
        This method records the calibration metadata as comments so that the
        provenance is captured inside the QASM artefact.
        """
        qubit_indices = sorted(self.calibration.keys())
        annotation = "// MeasurementErrorMitigation applied"
        if qubit_indices:
            annotation += f" — calibrated qubits: {qubit_indices}"
        return annotation + "\n" + qasm

    def description(self) -> str:
        return (
            "MeasurementErrorMitigation: invert per-qubit calibration matrix "
            "to remove readout-assignment errors from shot counts"
        )


# ── Strategy 2: Randomized Compiling (Pauli Twirling) ────────────────────────

# Twirling table for CNOT_{control, target}.
# Each entry is (pre_control, pre_target, post_control, post_target).
# The Pauli pairs satisfy:
#   (P_c ⊗ P_t) · CNOT · (Q_c ⊗ Q_t) = CNOT  (up to global phase)
#
# Derived from the relation CNOT · (A ⊗ B) · CNOT = (A·Z^b ⊗ X^a·B)
# where a = bit-action of A on control and b = phase-action of B on target.
_CNOT_TWIRL_TABLE: List[Tuple[str, str, str, str]] = [
    ("i",  "i",  "i",  "i"),
    ("x",  "i",  "x",  "x"),
    ("y",  "i",  "y",  "x"),
    ("z",  "i",  "z",  "i"),
    ("i",  "x",  "i",  "x"),
    ("x",  "x",  "x",  "i"),
    ("y",  "x",  "y",  "i"),
    ("z",  "x",  "z",  "x"),
    ("i",  "z",  "z",  "z"),
    ("x",  "z",  "y",  "z"),  # up to phase
    ("y",  "z",  "x",  "z"),  # up to phase
    ("z",  "z",  "i",  "z"),
    ("i",  "y",  "z",  "y"),
    ("x",  "y",  "y",  "y"),
    ("y",  "y",  "x",  "y"),
    ("z",  "y",  "i",  "y"),
]

# Regex to match a CNOT / CX gate line and capture control/target qubit refs.
_CNOT_RE = re.compile(
    r"^(cx|cnot)\s+(q\[\d+\])\s*,\s*(q\[\d+\])\s*;$",
    re.IGNORECASE,
)


class RandomizedCompiling(MitigationStrategy):
    """Apply Pauli twirling around CNOT gates to convert coherent gate errors
    into stochastic Pauli noise.

    For each CNOT gate in the circuit a random Pauli pair from the CNOT twirling
    group is inserted before the gate and the conjugate compensating Pauli pair
    is inserted after, so that the net logical operation is unchanged.  When
    averaged over many shots with different random seeds the coherent part of
    any CNOT error channel is projected out, leaving only a Pauli channel that
    is amenable to standard error-correction techniques.

    Parameters
    ----------
    n_samples:
        Number of randomized circuit variants to generate when
        :meth:`generate_twirled_circuits` is called (default 10).
    seed:
        Optional RNG seed for reproducibility.
    """

    def __init__(self, n_samples: int = 10, seed: int | None = None) -> None:
        self.n_samples = n_samples
        self._rng = random.Random(seed)

    def _twirl_qasm(self, qasm: str) -> str:
        """Return a single Pauli-twirled variant of *qasm*."""
        out_lines: List[str] = []
        for raw_line in qasm.splitlines():
            line = raw_line.strip()
            m = _CNOT_RE.match(line)
            if not m:
                out_lines.append(raw_line)
                continue

            ctrl, tgt = m.group(2), m.group(3)
            pre_c, pre_t, post_c, post_t = self._rng.choice(_CNOT_TWIRL_TABLE)

            def _gate_line(gate: str, qubit: str) -> str | None:
                if gate == "i":
                    return None
                return f"{gate} {qubit};"

            for gl in [
                _gate_line(pre_c, ctrl),
                _gate_line(pre_t, tgt),
                raw_line,
                _gate_line(post_c, ctrl),
                _gate_line(post_t, tgt),
            ]:
                if gl is not None:
                    out_lines.append(gl)

        return "\n".join(out_lines)

    def apply(self, qasm: str) -> str:
        """Return a single Pauli-twirled QASM circuit (one sample)."""
        return self._twirl_qasm(qasm)

    def generate_twirled_circuits(self, qasm: str) -> List[str]:
        """Return *n_samples* independent twirled circuit variants."""
        return [self._twirl_qasm(qasm) for _ in range(self.n_samples)]

    def description(self) -> str:
        return (
            "RandomizedCompiling: Pauli twirling around CNOT gates converts "
            "coherent gate errors into stochastic Pauli noise"
        )


# ── Convenience wrapper ───────────────────────────────────────────────────────

def mitigate(
    qasm: str,
    strategies: Sequence[MitigationStrategy],
) -> str:
    """Apply a sequence of mitigation strategies to *qasm* in order.

    Each strategy's :meth:`~MitigationStrategy.apply` is called in turn and
    the result is fed into the next strategy.
    """
    result = qasm
    for strategy in strategies:
        result = strategy.apply(result)
    return result


def available_strategies() -> Dict[str, Any]:
    """Return a mapping of strategy names to their classes."""
    return {
        "measurement_error_mitigation": MeasurementErrorMitigation,
        "randomized_compiling": RandomizedCompiling,
    }
