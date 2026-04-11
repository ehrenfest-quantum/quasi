# Example: 2-Site Fermi-Hubbard Model (Jordan-Wigner)

**File:** `hubbard_2site.cbor.hex`
**Size:** 510 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The 2-site Fermi-Hubbard model mapped to qubit operators via Jordan-Wigner
transformation. The Hubbard model describes strongly correlated electrons on a
lattice:

```
H = -t * sum_sigma (c+_0,sigma * c_1,sigma + h.c.) + U * sum_i n_i,up * n_i,down
```

With t=1.0, U=4.0 (strongly correlated regime), the Jordan-Wigner mapping to
4 qubits (2 sites x 2 spins) gives:

```
H = -0.5*(X_0 X_1 + Y_0 Y_1 + X_2 X_3 + Y_2 Y_3)  [hopping]
    + 1.0*Z_0 Z_2 - 1.0*Z_0 - 1.0*Z_2 + 1.0*I       [interaction]
```

**Measure:** Energy (E), sigma_z on qubit 0 (spin-up occupation at site 0)
**Evolution:** 2.0 us total, 20 Trotter steps (dt = 0.1 us)
**Noise floor:** T1 >= 1000 us, T2 >= 500 us (trapped-ion fidelity required)

## Physical Meaning

The Fermi-Hubbard model is central to condensed matter physics. At U/t = 4.0,
the system is in the strongly correlated regime where on-site Coulomb repulsion
dominates over kinetic energy (hopping). This ratio drives the Mott insulator
transition.

Qubits 0,1 encode spin-up electrons at sites 0,1; qubits 2,3 encode spin-down
electrons. The sigma_z observable on qubit 0 gives the spin-up occupation
at site 0: <Z_0> = 1 means empty, <Z_0> = -1 means occupied.

This is a minimal but physically meaningful example of a fermionic simulation
that requires high gate fidelity (trapped-ion noise parameters).

## Parameters

| Parameter        | Value   | Unit     |
|------------------|---------|----------|
| Qubits           | 4       |          |
| Sites            | 2       |          |
| Hopping t        | 1.0     | GHz*rad  |
| Interaction U    | 4.0     | GHz*rad  |
| U/t ratio        | 4.0     |          |
| Total time       | 2.0     | us       |
| Trotter steps    | 20      |          |
| T1 minimum       | 1000.0  | us       |
| T2 minimum       | 500.0   | us       |

## Expected Results

- Ground state energy: approximately -2.236 GHz*rad (exact diagonalization)
- At half filling, the system exhibits antiferromagnetic correlations
- <Z_0> oscillates between +/-1 during time evolution (charge dynamics)

## Python Reconstruction

```python
import cbor2

with open("hubbard_2site.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
print(f"Terms: {len(program['hamiltonian']['terms'])}")
print(f"Offset: {program['hamiltonian']['constant_offset']}")
# 7 terms + offset 1.0
```
