#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Valiant Quantum (Daniel Hinderink)
"""
Structural validator for Ehrenfest CBOR examples.
Validates QUASI-001 schema conformance without requiring the `cddl` Ruby gem.

Usage:
    python3 spec/tools/validate.py spec/examples/transverse_ising_2q.cbor.hex
    python3 spec/tools/validate.py spec/examples/   # validate all .cbor.hex files
"""

import binascii
import sys
from pathlib import Path

try:
    import cbor2
except ImportError:
    print("cbor2 required: pip install cbor2", file=sys.stderr)
    sys.exit(1)

REQUIRED_TOP_LEVEL = {"version", "system", "hamiltonian", "evolution", "observables", "noise"}
VALID_OBSERVABLE_TYPES = {"SZ", "SX", "E", "rho", "F"}
VALID_PAULI_AXES = {0, 1, 2, 3}


class ValidationError(Exception):
    pass


def check(cond: bool, msg: str) -> None:
    if not cond:
        raise ValidationError(msg)


def validate_program(p: dict) -> None:
    """Validate a decoded EhrenfestProgram dict against the v0.1 schema."""

    # Top-level fields
    check(isinstance(p, dict), "root must be a map")
    missing = REQUIRED_TOP_LEVEL - set(p.keys())
    check(not missing, f"missing required top-level fields: {missing}")

    # version
    check(isinstance(p["version"], int), "version must be uint")
    check(p["version"] == 1, f"version must be 1 for v0.1, got {p['version']}")

    # system
    sys_ = p["system"]
    check(isinstance(sys_, dict), "system must be a map")
    check("n_qubits" in sys_, "system.n_qubits is required")
    check(isinstance(sys_["n_qubits"], int) and sys_["n_qubits"] > 0,
          "system.n_qubits must be a positive uint")
    n_qubits = sys_["n_qubits"]

    if "cooling_profile" in sys_:
        cp = sys_["cooling_profile"]
        check(isinstance(cp, dict), "cooling_profile must be a map")
        check("target_temp_mk" in cp, "cooling_profile.target_temp_mk is required")
        check(isinstance(cp["target_temp_mk"], (int, float)),
              "cooling_profile.target_temp_mk must be a float")

    # hamiltonian
    h = p["hamiltonian"]
    check(isinstance(h, dict), "hamiltonian must be a map")
    check("terms" in h, "hamiltonian.terms is required")
    check("constant_offset" in h, "hamiltonian.constant_offset is required")
    check(isinstance(h["terms"], list) and len(h["terms"]) >= 1,
          "hamiltonian.terms must be a non-empty array")
    check(isinstance(h["constant_offset"], (int, float)),
          "hamiltonian.constant_offset must be a float")

    for i, term in enumerate(h["terms"]):
        check(isinstance(term, dict), f"hamiltonian.terms[{i}] must be a map")
        check("coefficient" in term, f"hamiltonian.terms[{i}].coefficient is required")
        check("paulis" in term, f"hamiltonian.terms[{i}].paulis is required")
        check(isinstance(term["paulis"], list),
              f"hamiltonian.terms[{i}].paulis must be an array")
        for j, op in enumerate(term["paulis"]):
            check(isinstance(op, dict), f"paulis[{j}] must be a map")
            check("qubit" in op and "axis" in op,
                  f"paulis[{j}] must have qubit and axis")
            check(isinstance(op["qubit"], int) and 0 <= op["qubit"] < n_qubits,
                  f"paulis[{j}].qubit={op['qubit']} out of range [0, {n_qubits})")
            check(op["axis"] in VALID_PAULI_AXES,
                  f"paulis[{j}].axis={op['axis']} must be 0/1/2/3")

    # evolution
    evo = p["evolution"]
    check(isinstance(evo, dict), "evolution must be a map")
    for field in ("total_us", "steps", "dt_us"):
        check(field in evo, f"evolution.{field} is required")
    check(isinstance(evo["steps"], int) and evo["steps"] >= 1,
          "evolution.steps must be a positive uint")
    check(isinstance(evo["total_us"], (int, float)) and evo["total_us"] > 0,
          "evolution.total_us must be a positive float")
    check(isinstance(evo["dt_us"], (int, float)) and evo["dt_us"] > 0,
          "evolution.dt_us must be a positive float")
    # dt consistency check (allow 1% tolerance for float rounding)
    expected_dt = evo["total_us"] / evo["steps"]
    check(abs(evo["dt_us"] - expected_dt) / expected_dt < 0.01,
          f"evolution.dt_us={evo['dt_us']} inconsistent with total_us/steps={expected_dt:.6f}")

    # observables
    obs = p["observables"]
    check(isinstance(obs, list) and len(obs) >= 1,
          "observables must be a non-empty array")
    for i, o in enumerate(obs):
        check(isinstance(o, dict), f"observables[{i}] must be a map")
        check("type" in o, f"observables[{i}].type is required")
        check(o["type"] in VALID_OBSERVABLE_TYPES,
              f"observables[{i}].type={o['type']!r} must be one of {VALID_OBSERVABLE_TYPES}")
        if o["type"] in ("SZ", "SX"):
            check("qubit" in o, f"observables[{i}] (type={o['type']}) requires qubit field")
            check(0 <= o["qubit"] < n_qubits,
                  f"observables[{i}].qubit={o['qubit']} out of range")
        if o["type"] == "rho":
            check("qubits" in o and isinstance(o["qubits"], list) and len(o["qubits"]) >= 1,
                  f"observables[{i}] (type=rho) requires non-empty qubits array")
        if o["type"] == "F":
            check("target_state" in o and isinstance(o["target_state"], bytes),
                  f"observables[{i}] (type=F) requires target_state as bstr")

    # noise
    noise = p["noise"]
    check(isinstance(noise, dict), "noise must be a map")
    check("t1_us" in noise, "noise.t1_us is REQUIRED")
    check("t2_us" in noise, "noise.t2_us is REQUIRED")
    check(isinstance(noise["t1_us"], (int, float)) and noise["t1_us"] > 0,
          "noise.t1_us must be a positive float")
    check(isinstance(noise["t2_us"], (int, float)) and noise["t2_us"] > 0,
          "noise.t2_us must be a positive float")
    check(noise["t2_us"] <= 2 * noise["t1_us"],
          f"noise.t2_us={noise['t2_us']} violates physical bound T2 ≤ 2·T1={noise['t1_us']}")

    if "gate_fidelity_min" in noise:
        f = noise["gate_fidelity_min"]
        check(isinstance(f, (int, float)) and 0.0 <= f <= 1.0,
              f"noise.gate_fidelity_min={f} must be in [0.0, 1.0]")
    if "readout_fidelity_min" in noise:
        f = noise["readout_fidelity_min"]
        check(isinstance(f, (int, float)) and 0.0 <= f <= 1.0,
              f"noise.readout_fidelity_min={f} must be in [0.0, 1.0]")


def validate_file(path: Path) -> bool:
    try:
        raw = bytes.fromhex(path.read_text().strip())
        program = cbor2.loads(raw)
        validate_program(program)
        n = program["system"]["n_qubits"]
        t = len(program["hamiltonian"]["terms"])
        obs = program["observables"][0]["type"]
        print(f"  ✓  {path.name}  ({len(raw)}B, {n}q, {t} terms, obs={obs})")
        return True
    except ValidationError as e:
        print(f"  ✗  {path.name}: {e}")
        return False
    except Exception as e:
        print(f"  ✗  {path.name}: decode error: {e}")
        return False


def main() -> None:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <file.cbor.hex> | <directory>")
        sys.exit(1)

    target = Path(sys.argv[1])
    if target.is_dir():
        files = sorted(target.glob("*.cbor.hex"))
        if not files:
            print(f"No .cbor.hex files found in {target}")
            sys.exit(1)
    else:
        files = [target]

    print(f"Validating {len(files)} file(s) against Ehrenfest v0.1 schema...")
    results = [validate_file(f) for f in files]
    passed = sum(results)
    print(f"\n{passed}/{len(results)} passed")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
