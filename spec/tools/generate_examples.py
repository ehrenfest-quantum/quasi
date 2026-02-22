#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Valiant Quantum (Daniel Hinderink)
"""
Generate canonical CBOR examples for QUASI-001 Ehrenfest schema.

Writes hex-encoded CBOR to spec/examples/
Run from repo root: python3 spec/tools/generate_examples.py
"""

import binascii
import json
import sys
from pathlib import Path

try:
    import cbor2
except ImportError:
    print("cbor2 required: pip install cbor2", file=sys.stderr)
    sys.exit(1)

EXAMPLES_DIR = Path(__file__).parent.parent / "examples"
EXAMPLES_DIR.mkdir(parents=True, exist_ok=True)


def encode(program: dict) -> bytes:
    return cbor2.dumps(program, canonical=True)


def write_example(name: str, program: dict) -> None:
    raw = encode(program)
    hex_str = binascii.hexlify(raw).decode()
    path = EXAMPLES_DIR / f"{name}.cbor.hex"
    path.write_text(hex_str + "\n")
    print(f"  {path.name}  ({len(raw)} bytes)")
    return raw


# ── Example 1: Transverse-Field Ising Model, 2 qubits ────────────────────────
#
# H = -J·Z₀Z₁ - h·(X₀ + X₁),  J = 1.0 GHz·rad, h = 0.5 GHz·rad
# Measure: σᶻ on q0 and q1
# Evolution: 1 μs, 10 Trotter steps (dt = 0.1 μs)
# Noise: T1 = 100 μs, T2 = 80 μs, gate fidelity ≥ 0.999
#
# Physical meaning: spin chain near the quantum critical point (h/J = 0.5).
# The ZZ coupling creates entanglement; the transverse field drives spin flips.

transverse_ising_2q = {
    "version": 1,
    "system": {
        "n_qubits": 2,
        "backend_hint": "ibm_torino",
    },
    "hamiltonian": {
        "terms": [
            # -J·Z₀Z₁
            {
                "coefficient": -1.0,
                "paulis": [
                    {"qubit": 0, "axis": 3},  # Z
                    {"qubit": 1, "axis": 3},  # Z
                ],
            },
            # -h·X₀
            {
                "coefficient": -0.5,
                "paulis": [{"qubit": 0, "axis": 1}],  # X
            },
            # -h·X₁
            {
                "coefficient": -0.5,
                "paulis": [{"qubit": 1, "axis": 1}],  # X
            },
        ],
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 1.0,
        "steps": 10,
        "dt_us": 0.1,
    },
    "observables": [
        {"type": "SZ", "qubit": 0},
        {"type": "SZ", "qubit": 1},
    ],
    "noise": {
        "t1_us": 100.0,
        "t2_us": 80.0,
        "gate_fidelity_min": 0.999,
    },
}

# ── Example 2: Rabi Oscillation, 1 qubit ─────────────────────────────────────
#
# H = -Ω/2·X,  Ω = 2π × 5 MHz = 31.416 MHz → 0.031416 GHz·rad
# Measure: σˣ (tracks the oscillation)
# Evolution: 0.1 μs, 5 Trotter steps (dt = 0.02 μs)
# Noise: T1 = 50 μs, T2 = 30 μs
#
# Physical meaning: single qubit driven at its resonance frequency.
# σˣ expectation oscillates as cos(Ω·t) — the textbook Rabi oscillation.

rabi_oscillation_1q = {
    "version": 1,
    "system": {
        "n_qubits": 1,
    },
    "hamiltonian": {
        "terms": [
            # -Ω/2·X  (Ω = 2π × 5 MHz in GHz·rad units)
            {
                "coefficient": -0.015708,   # -π × 0.005 GHz·rad
                "paulis": [{"qubit": 0, "axis": 1}],  # X
            },
        ],
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 0.1,
        "steps": 5,
        "dt_us": 0.02,
    },
    "observables": [
        {"type": "SX", "qubit": 0},
    ],
    "noise": {
        "t1_us": 50.0,
        "t2_us": 30.0,
    },
}

# ── Example 3: Heisenberg XXX Model, 4 qubits ────────────────────────────────
#
# H = J·Σ⟨i,j⟩(X_i X_j + Y_i Y_j + Z_i Z_j),  J = 1.0 GHz·rad
# Nearest-neighbor chain: (0,1), (1,2), (2,3)
# Measure: Energy ⟨ψ|H|ψ⟩
# Evolution: 2 μs, 20 Trotter steps (dt = 0.1 μs)
# Noise: T1 = 150 μs, T2 = 100 μs, gate ≥ 0.9995, readout ≥ 0.995
#
# Physical meaning: SU(2)-symmetric spin chain — relevant for quantum magnetism
# and as a benchmark for quantum advantage in ground-state energy estimation.

heisenberg_4q = {
    "version": 1,
    "system": {
        "n_qubits": 4,
    },
    "hamiltonian": {
        "terms": [
            # XX nearest-neighbor pairs
            {"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 1}, {"qubit": 1, "axis": 1}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 1, "axis": 1}, {"qubit": 2, "axis": 1}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 2, "axis": 1}, {"qubit": 3, "axis": 1}]},
            # YY nearest-neighbor pairs
            {"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 2}, {"qubit": 1, "axis": 2}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 1, "axis": 2}, {"qubit": 2, "axis": 2}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 2, "axis": 2}, {"qubit": 3, "axis": 2}]},
            # ZZ nearest-neighbor pairs
            {"coefficient": 1.0, "paulis": [{"qubit": 0, "axis": 3}, {"qubit": 1, "axis": 3}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 1, "axis": 3}, {"qubit": 2, "axis": 3}]},
            {"coefficient": 1.0, "paulis": [{"qubit": 2, "axis": 3}, {"qubit": 3, "axis": 3}]},
        ],
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 2.0,
        "steps": 20,
        "dt_us": 0.1,
    },
    "observables": [
        {"type": "E"},
    ],
    "noise": {
        "t1_us": 150.0,
        "t2_us": 100.0,
        "gate_fidelity_min": 0.9995,
        "readout_fidelity_min": 0.995,
    },
}


def main() -> None:
    print("Generating Ehrenfest CBOR examples...")
    examples = [
        ("transverse_ising_2q", transverse_ising_2q),
        ("rabi_oscillation_1q", rabi_oscillation_1q),
        ("heisenberg_4q", heisenberg_4q),
    ]
    for name, program in examples:
        write_example(name, program)
    print("Done.")


if __name__ == "__main__":
    main()
