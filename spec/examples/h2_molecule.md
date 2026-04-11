# Example: H2 Molecule (STO-3G Basis, Jordan-Wigner)

**File:** `h2_molecule.cbor.hex`
**Size:** 420 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The hydrogen molecule Hamiltonian in the STO-3G minimal basis set at
equilibrium bond length (0.735 Angstrom), mapped to qubit operators via the
Jordan-Wigner transformation:

```
H = -1.0523*I + 0.3979*Z_1 - 0.3979*Z_0 - 0.0113*Z_0*Z_1
    + 0.1809*X_0*X_1 + 0.1809*Y_0*Y_1
```

**Measure:** Energy (E) -- ground state energy of H2
**Evolution:** 5.0 us total, 20 Trotter steps (dt = 0.25 us)
**Noise floor:** T1 >= 1000 us, T2 >= 500 us (trapped-ion fidelity required)

## Physical Meaning

The H2 molecule is the simplest molecular system and the canonical benchmark
for quantum chemistry on quantum computers. The STO-3G basis with 2 molecular
orbitals maps to 2 qubits via Jordan-Wigner.

The five Pauli terms encode the electronic structure:
- **Z terms (0.3979, -0.3979):** One-electron integrals (kinetic + nuclear attraction)
- **ZZ term (-0.0113):** Diagonal two-electron repulsion
- **XX and YY terms (0.1809):** Off-diagonal electron correlation (the "hard" part)
- **Constant offset (-1.0523):** Nuclear repulsion + core energy

This Hamiltonian is the standard input for VQE (Variational Quantum
Eigensolver). The Trotterized time evolution approximates the ground state
energy through quantum phase estimation or variational optimization.

## Parameters

| Parameter        | Value       | Unit      |
|------------------|-------------|-----------|
| Qubits           | 2           |           |
| Bond length      | 0.735       | Angstrom  |
| Basis set        | STO-3G      |           |
| Pauli terms      | 5           |           |
| Constant offset  | -1.0523     | Hartree   |
| Total time       | 5.0         | us        |
| Trotter steps    | 20          |           |
| T1 minimum       | 1000.0      | us        |
| T2 minimum       | 500.0       | us        |

## Expected Results

- Exact ground state energy: -1.137 Hartree (at equilibrium geometry)
- VQE should converge within ~50 iterations with COBYLA optimizer
- Optimal theta ~ pi, phi ~ 0 (Hartree-Fock reference + correlation correction)

## Python Reconstruction

```python
import cbor2

with open("h2_molecule.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
print(f"Constant offset: {program['hamiltonian']['constant_offset']}")
for term in program["hamiltonian"]["terms"]:
    axes = {0: "I", 1: "X", 2: "Y", 3: "Z"}
    ops = " ".join(f"{axes[p['axis']]}{p['qubit']}" for p in term["paulis"])
    print(f"  {term['coefficient']:+.4f} * {ops}")
# -1.0523 (offset) + 0.3979*Z1 - 0.3979*Z0 - 0.0113*Z0Z1 + 0.1809*X0X1 + 0.1809*Y0Y1
```
