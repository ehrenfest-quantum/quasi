# Example: GHZ State Verification, 8 Qubits

**File:** `ghz_8q.cbor.hex`
**Size:** 642 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The ZZ chain Hamiltonian whose ground state is the GHZ state:

```
H = Z_0 Z_1 + Z_1 Z_2 + Z_2 Z_3 + Z_3 Z_4 + Z_4 Z_5 + Z_5 Z_6 + Z_6 Z_7
```

This is a nearest-neighbor ferromagnetic Ising chain (7 ZZ terms, all with
coefficient +1.0).

**Measure:** sigma_z on all 8 qubits (full magnetization profile)
**Evolution:** 0.5 us total, 5 Trotter steps (dt = 0.1 us)
**Noise floor:** T1 >= 200 us, T2 >= 150 us

## Physical Meaning

The GHZ (Greenberger-Horne-Zeilinger) state |00000000> + |11111111> is a
maximally entangled 8-qubit state. It is the ground state of this ferromagnetic
ZZ chain.

The 8 sigma_z observables verify that the qubits are correlated: in the GHZ
state, all qubits should show <Z_i> = 0 individually (equal superposition of
|0> and |1>), but Z_i Z_j = +1 for all pairs (perfect correlation).

GHZ states are the standard benchmark for multi-qubit entanglement fidelity
and are used in quantum error correction, quantum metrology, and tests of
Bell inequality violations.

## Parameters

| Parameter        | Value   | Unit     |
|------------------|---------|----------|
| Qubits           | 8       |          |
| ZZ terms         | 7       |          |
| ZZ coefficient   | 1.0     | GHz*rad  |
| Constant offset  | 0.0     | GHz*rad  |
| Total time       | 0.5     | us       |
| Trotter steps    | 5       |          |
| T1 minimum       | 200.0   | us       |
| T2 minimum       | 150.0   | us       |

## Expected Results

- Ground state: |00000000> + |11111111> (2-fold degenerate)
- Ground state energy: -7.0 GHz*rad
- Individual <Z_i> = 0 for all qubits
- <Z_i Z_j> = +1 for all nearest-neighbor pairs

## Python Reconstruction

```python
import cbor2

with open("ghz_8q.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
print(f"Qubits: {program['system']['n_qubits']}")
print(f"ZZ terms: {len(program['hamiltonian']['terms'])}")
print(f"Observables: {len(program['observables'])} (SZ on each qubit)")
```
