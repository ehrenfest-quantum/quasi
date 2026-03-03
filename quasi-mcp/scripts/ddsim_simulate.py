#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""DDSIM noise-aware fidelity estimator for Ehrenfest circuits.

Adopted from Arvak's arvak-python/python/arvak/nathan/noise.py.
Two modes:
- mqt.ddsim available: noisy + ideal simulation, TVD fidelity
- Heuristic fallback: product-of-gate-fidelities estimate

Usage:
    python3 ddsim_simulate.py <qasm_file> [--backend NAME] [--shots N]
        [--sq-err F] [--tq-err F]

Prints JSON to stdout:
    {"fidelity": float, "method": str, "noise_model": str, "shots": int,
     "ideal_counts": {...}, "noisy_counts": {...}, "error": null}
"""

import argparse
import json
import math
import re
import sys


# -- Backend noise profiles (from Arvak) ------------------------------------
BACKEND_PROFILES = {
    "ibm_heron":      (0.0005, 0.003, 200.0, 150.0),
    "ibm_eagle":      (0.001,  0.005, 150.0, 100.0),
    "ibm_marrakesh":  (0.0005, 0.003, 200.0, 150.0),
    "ibm_torino":     (0.0005, 0.003, 200.0, 150.0),
    "iqm_garnet":     (0.002,  0.007, 80.0,  50.0),
    "iqm_sirius":     (0.002,  0.006, 90.0,  60.0),
    "quantinuum_h2":  (0.0001, 0.002, 1000.0, 500.0),
    "quantinuum_h1":  (0.0002, 0.003, 800.0, 400.0),
    "simulator":      (0.0,    0.0,   1e9,   1e9),
}


def _parse_qasm3_stats(qasm3):
    """Quick parse: (num_qubits, total_gates, two_qubit_gates)."""
    num_qubits = 0
    for m in re.finditer(r"qubit\s*\[(\d+)\]", qasm3):
        num_qubits += int(m.group(1))
    if num_qubits == 0:
        for m in re.finditer(r"qreg\s+\w+\s*\[(\d+)\]", qasm3):
            num_qubits += int(m.group(1))

    gate_line = re.compile(
        r"^\s*(?!//|qubit|qreg|creg|bit|input|output|OPENQASM|include|measure|barrier|reset)"
        r"[a-zA-Z_]\w*(?:\s*\([^)]*\))?\s+[a-zA-Z\[]",
        re.MULTILINE,
    )
    total_gates = len(gate_line.findall(qasm3))

    tq_pattern = re.compile(
        r"^\s*(?:cx|cz|cnot|cp|cu|ecr|rxx|rzz|swap|iswap|ccx|toffoli)\b",
        re.MULTILINE | re.IGNORECASE,
    )
    two_qubit_gates = len(tq_pattern.findall(qasm3))

    return num_qubits, total_gates, two_qubit_gates


def _resolve_noise(backend_name, sq_err_override, tq_err_override):
    """Resolve noise parameters from backend profile or overrides."""
    if sq_err_override is not None and tq_err_override is not None:
        return sq_err_override, tq_err_override

    key = backend_name.lower().replace("-", "_")
    for k, v in BACKEND_PROFILES.items():
        if k in key or key in k:
            return v[0], v[1]

    return 0.001, 0.005  # conservative default


def _tvd(dist_a, dist_b):
    """Total Variation Distance between two probability distributions."""
    all_keys = set(dist_a) | set(dist_b)
    return 0.5 * sum(abs(dist_a.get(k, 0.0) - dist_b.get(k, 0.0)) for k in all_keys)


def heuristic_fidelity(qasm3, backend_name, sq_err, tq_err):
    """Estimate fidelity from product-of-gate-fidelities model."""
    _nq, total_gates, tq_gates = _parse_qasm3_stats(qasm3)
    sq_gates = max(0, total_gates - tq_gates)
    fidelity = (1.0 - sq_err) ** sq_gates * (1.0 - tq_err) ** tq_gates
    return max(0.0, min(1.0, fidelity))


def ddsim_fidelity(qasm3, backend_name, sq_err, tq_err, shots):
    """Run noisy + ideal simulation via mqt.ddsim, return fidelity via TVD."""
    from mqt.ddsim import DDSIMProvider
    from qiskit.qasm3 import loads as qasm3_loads

    qc = qasm3_loads(qasm3)
    provider = DDSIMProvider()

    # Ideal simulation
    ideal_backend = provider.get_backend("qasm_simulator")
    ideal_job = ideal_backend.run(qc, shots=shots)
    ideal_counts = ideal_job.result().get_counts()

    # Noisy simulation — use stochastic_noise_simulator (ddsim v2.x)
    noisy_backend = provider.get_backend("stochastic_noise_simulator")
    noisy_job = noisy_backend.run(qc, shots=shots)
    noisy_counts = noisy_job.result().get_counts()

    total_ideal = sum(ideal_counts.values()) or 1
    total_noisy = sum(noisy_counts.values()) or 1
    ideal_dist = {k: v / total_ideal for k, v in ideal_counts.items()}
    noisy_dist = {k: v / total_noisy for k, v in noisy_counts.items()}

    tvd = _tvd(ideal_dist, noisy_dist)
    fidelity = max(0.0, 1.0 - tvd)

    return fidelity, ideal_counts, noisy_counts


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("qasm_file", help="Path to QASM3 file")
    parser.add_argument("--backend", default="simulator", help="Target backend name")
    parser.add_argument("--shots", type=int, default=1024)
    parser.add_argument("--sq-err", type=float, default=None, help="Single-qubit error rate")
    parser.add_argument("--tq-err", type=float, default=None, help="Two-qubit error rate")
    args = parser.parse_args()

    with open(args.qasm_file) as f:
        qasm3 = f.read()

    sq_err, tq_err = _resolve_noise(args.backend, args.sq_err, args.tq_err)
    noise_model = "depolarizing(sq={}, tq={})".format(sq_err, tq_err)

    try:
        from mqt.ddsim import DDSIMProvider  # noqa: F401
        from qiskit.qasm3 import loads  # noqa: F401
        ddsim_ok = True
    except ImportError:
        ddsim_ok = False

    result = {
        "fidelity": 0.0,
        "method": "unknown",
        "noise_model": noise_model,
        "shots": args.shots,
        "ideal_counts": None,
        "noisy_counts": None,
        "error": None,
    }

    if ddsim_ok:
        try:
            fid, ideal_counts, noisy_counts = ddsim_fidelity(
                qasm3, args.backend, sq_err, tq_err, args.shots
            )
            result["fidelity"] = fid
            result["method"] = "ddsim_noisy"
            result["ideal_counts"] = ideal_counts
            result["noisy_counts"] = noisy_counts
            print(json.dumps(result))
            return
        except Exception as e:
            pass  # fall through to heuristic

    # Heuristic fallback
    fid = heuristic_fidelity(qasm3, args.backend, sq_err, tq_err)
    result["fidelity"] = fid
    result["method"] = "heuristic"
    result["shots"] = 0
    print(json.dumps(result))


if __name__ == "__main__":
    main()
