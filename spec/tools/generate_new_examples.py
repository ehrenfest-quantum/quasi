#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Daniel Hinderink
"""
Generate new Ehrenfest CBOR example programs for QUASI-001.

Writes hex-encoded CBOR to spec/examples/{name}.cbor.hex
and documentation to spec/examples/{name}.md

Run from repo root: python3 spec/tools/generate_new_examples.py
"""

import binascii
import sys
from pathlib import Path

try:
    import cbor2
except ImportError:
    print("cbor2 required: pip install cbor2", file=sys.stderr)
    sys.exit(1)

SPEC_EXAMPLES_DIR = Path(__file__).parent.parent / "examples"
SPEC_EXAMPLES_DIR.mkdir(parents=True, exist_ok=True)


def encode(program: dict) -> bytes:
    return cbor2.dumps(program, canonical=True)


def write_example(name: str, program: dict) -> bytes:
    raw = encode(program)
    hex_str = binascii.hexlify(raw).decode()
    hex_path = SPEC_EXAMPLES_DIR / f"{name}.cbor.hex"
    hex_path.write_text(hex_str + "\n")
    print(f"  {hex_path.relative_to(SPEC_EXAMPLES_DIR.parent.parent)}  ({len(raw)} bytes)")
    return raw


def write_doc(name: str, content: str) -> None:
    doc_path = SPEC_EXAMPLES_DIR / f"{name}.md"
    doc_path.write_text(content)
    print(f"  {doc_path.relative_to(SPEC_EXAMPLES_DIR.parent.parent)}")


# ── Helper: ZZ term ──────────────────────────────────────────────────────────

def zz_term(q1: int, q2: int, coeff: float) -> dict:
    return {"coefficient": coeff, "paulis": [{"qubit": q1, "axis": 3}, {"qubit": q2, "axis": 3}]}


def xx_term(q1: int, q2: int, coeff: float) -> dict:
    return {"coefficient": coeff, "paulis": [{"qubit": q1, "axis": 1}, {"qubit": q2, "axis": 1}]}


def yy_term(q1: int, q2: int, coeff: float) -> dict:
    return {"coefficient": coeff, "paulis": [{"qubit": q1, "axis": 2}, {"qubit": q2, "axis": 2}]}


def z_term(q: int, coeff: float) -> dict:
    return {"coefficient": coeff, "paulis": [{"qubit": q, "axis": 3}]}


def x_term(q: int, coeff: float) -> dict:
    return {"coefficient": coeff, "paulis": [{"qubit": q, "axis": 1}]}


# ═══════════════════════════════════════════════════════════════════════════════
# Example 1: QAOA MaxCut on 4-node cycle graph
# ═══════════════════════════════════════════════════════════════════════════════

qaoa_maxcut_4q = {
    "version": 1,
    "system": {"n_qubits": 4},
    "hamiltonian": {
        "terms": [
            # -(1/2) Z₀Z₁, -(1/2) Z₁Z₂, -(1/2) Z₂Z₃, -(1/2) Z₃Z₀
            zz_term(0, 1, -0.5),
            zz_term(1, 2, -0.5),
            zz_term(2, 3, -0.5),
            zz_term(3, 0, -0.5),
        ],
        "constant_offset": 2.0,
    },
    "evolution": {
        "total_us": 1.0,
        "steps": 10,
        "dt_us": 0.1,
    },
    "observables": [{"type": "E"}],
    "noise": {
        "t1_us": 200.0,
        "t2_us": 150.0,
    },
}

QAOA_DOC = """\
# Example: QAOA MaxCut, 4-Node Cycle Graph

**File:** `qaoa_maxcut_4q.cbor.hex`
**Size:** 366 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

QAOA cost Hamiltonian for the MaxCut problem on a 4-node cycle graph (0-1-2-3-0):

```
H_C = (1/2)(I - Z_0 Z_1) + (1/2)(I - Z_1 Z_2) + (1/2)(I - Z_2 Z_3) + (1/2)(I - Z_3 Z_0)
    = 2*I - (1/2)(Z_0 Z_1 + Z_1 Z_2 + Z_2 Z_3 + Z_3 Z_0)
```

**Measure:** Energy (E) -- expectation value of H_C gives the cut value
**Evolution:** 1.0 us total, 10 Trotter steps (dt = 0.1 us)
**Noise floor:** T1 >= 200 us, T2 >= 150 us (superconducting)

## Physical Meaning

MaxCut partitions graph vertices into two sets to maximize the number of edges
crossing the partition. For the 4-node cycle, the optimal cut has value 4 (all
edges cut), achieved by alternating partition assignments (0,1,0,1).

The QAOA cost Hamiltonian encodes this: its ground state energy equals the
negative of the maximum cut value. The constant offset of 2.0 shifts the
spectrum so that the ground state energy corresponds to the cut value directly.

This is the standard benchmark for QAOA on near-term quantum hardware.

## Parameters

| Parameter        | Value   | Unit     |
|------------------|---------|----------|
| Qubits           | 4       |          |
| ZZ terms         | 4       |          |
| ZZ coefficient   | -0.5    | GHz*rad  |
| Constant offset  | 2.0     | GHz*rad  |
| Total time       | 1.0     | us       |
| Trotter steps    | 10      |          |
| T1 minimum       | 200.0   | us       |
| T2 minimum       | 150.0   | us       |

## Expected Results

- Maximum cut value: 4 (all edges of the cycle are cut)
- Ground state energy of H_C: 0.0 (with offset, minimum eigenvalue = 0)
- Optimal bitstring: |0101> or |1010> (alternating partition)

## Python Reconstruction

```python
import cbor2

with open("qaoa_maxcut_4q.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
for term in program["hamiltonian"]["terms"]:
    qubits = [p["qubit"] for p in term["paulis"]]
    print(f"  coeff={term['coefficient']}, qubits={qubits}")
# Each term: coeff=-0.5, ZZ on pairs (0,1), (1,2), (2,3), (3,0)
```
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Example 2: 2-site Fermi-Hubbard model (Jordan-Wigner)
# ═══════════════════════════════════════════════════════════════════════════════

hubbard_2site = {
    "version": 1,
    "system": {"n_qubits": 4},
    "hamiltonian": {
        "terms": [
            # Hopping: -t/2 (X_0 X_1 + Y_0 Y_1 + X_2 X_3 + Y_2 Y_3), t=1.0
            xx_term(0, 1, -0.5),
            yy_term(0, 1, -0.5),
            xx_term(2, 3, -0.5),
            yy_term(2, 3, -0.5),
            # Interaction: U/4 (Z_0 Z_2), U=4.0
            zz_term(0, 2, 1.0),
            # Interaction: -U/4 (Z_0 + Z_2), U=4.0
            z_term(0, -1.0),
            z_term(2, -1.0),
        ],
        "constant_offset": 1.0,  # U/4 = 1.0
    },
    "evolution": {
        "total_us": 2.0,
        "steps": 20,
        "dt_us": 0.1,
    },
    "observables": [
        {"type": "E"},
        {"type": "SZ", "qubit": 0},
    ],
    "noise": {
        "t1_us": 1000.0,
        "t2_us": 500.0,
    },
}

HUBBARD_DOC = """\
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
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Example 3: GHZ-state verification Hamiltonian, 8 qubits
# ═══════════════════════════════════════════════════════════════════════════════

ghz_8q = {
    "version": 1,
    "system": {"n_qubits": 8},
    "hamiltonian": {
        "terms": [zz_term(i, i + 1, 1.0) for i in range(7)],
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 0.5,
        "steps": 5,
        "dt_us": 0.1,
    },
    "observables": [{"type": "SZ", "qubit": i} for i in range(8)],
    "noise": {
        "t1_us": 200.0,
        "t2_us": 150.0,
    },
}

GHZ_DOC = """\
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
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Example 4: Solvayeur 4-backend scheduling Hamiltonian
# ═══════════════════════════════════════════════════════════════════════════════

solvayeur_4backend = {
    "version": 1,
    "system": {"n_qubits": 2},
    "hamiltonian": {
        "terms": [
            # J * Z_0 Z_1 -- contention coupling
            zz_term(0, 1, 0.1),
            # h_0 * Z_0 -- bias from previous scheduling rounds
            z_term(0, 0.3),
            # h_1 * Z_1 -- bias from previous scheduling rounds
            z_term(1, 0.2),
            # Gamma * X_0 -- exploration (quantum tunneling)
            x_term(0, 0.5),
            # Gamma * X_1 -- exploration (quantum tunneling)
            x_term(1, 0.5),
        ],
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 1.0,
        "steps": 5,
        "dt_us": 0.2,
    },
    "observables": [
        {"type": "SZ", "qubit": 0},
        {"type": "SZ", "qubit": 1},
    ],
    "noise": {
        "t1_us": 200.0,
        "t2_us": 150.0,
    },
}

SOLVAYEUR_DOC = """\
# Example: Solvayeur 4-Backend Scheduling Hamiltonian

**File:** `solvayeur_4backend.cbor.hex`
**Size:** 404 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The Solvayeur scheduling kernel: an Ehrenfest program that encodes a quantum
annealing-inspired scheduling decision across 4 quantum backends.

```
H = J*Z_0*Z_1 + h_0*Z_0 + h_1*Z_1 + Gamma*X_0 + Gamma*X_1
```

With:
- J = 0.1 (contention coupling between backend pairs)
- h_0 = 0.3 (bias from previous scheduling round for backend pair 0)
- h_1 = 0.2 (bias from previous scheduling round for backend pair 1)
- Gamma = 0.5 (exploration strength -- quantum tunneling between states)

**Measure:** sigma_z on qubit 0 and qubit 1 (dispatching decision)
**Evolution:** 1.0 us total, 5 Trotter steps (dt = 0.2 us) -- shallow circuit
**Noise floor:** T1 >= 200 us, T2 >= 150 us

## Physical Meaning -- The OS Kernel

This is the Solvayeur kernel: a quantum operating system scheduler that uses
Ehrenfest physics programs to make dispatching decisions. Two qubits encode
four backends via binary encoding:

| Qubit 0 | Qubit 1 | Backend        |
|---------|---------|----------------|
| |0>     | |0>     | Backend A      |
| |0>     | |1>     | Backend B      |
| |1>     | |0>     | Backend C      |
| |1>     | |1>     | Backend D      |

The Hamiltonian terms encode scheduling physics:

- **ZZ coupling (J=0.1):** Contention -- backends that share resources incur
  an energy penalty when both are selected. Low J means weak contention.
- **Z biases (h_0, h_1):** Historical performance -- backends that performed
  well in previous rounds get a lower energy (higher selection probability).
  h_0=0.3 means backend pair 0 has stronger historical preference.
- **X terms (Gamma=0.5):** Exploration -- quantum tunneling allows the
  scheduler to explore non-obvious backend assignments. Without this term
  the scheduler would be a classical greedy algorithm.

The sigma_z measurements on both qubits after evolution produce the dispatching
decision. The ground state of H encodes the optimal backend assignment given
the current contention and historical data.

This proves that Ehrenfest programs can express not just physics simulations
but also combinatorial optimization problems -- including the QUASI operating
system's own scheduling kernel.

## Parameters

| Parameter              | Value   | Unit     |
|------------------------|---------|----------|
| Qubits                 | 2       |          |
| Backends encoded       | 4       |          |
| Contention J           | 0.1     | GHz*rad  |
| Bias h_0               | 0.3     | GHz*rad  |
| Bias h_1               | 0.2     | GHz*rad  |
| Exploration Gamma      | 0.5     | GHz*rad  |
| Total time             | 1.0     | us       |
| Trotter steps          | 5       |          |
| T1 minimum             | 200.0   | us       |
| T2 minimum             | 150.0   | us       |

## Expected Results

- Ground state: determined by competition between bias and exploration
- <Z_0> and <Z_1> give the dispatching decision
- With these parameters, the scheduler should prefer backends with lower
  energy (stronger historical bias), modulated by exploration

## Python Reconstruction

```python
import cbor2

with open("solvayeur_4backend.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
for term in program["hamiltonian"]["terms"]:
    axes = {0: "I", 1: "X", 2: "Y", 3: "Z"}
    ops = "".join(f"{axes[p['axis']]}{p['qubit']}" for p in term["paulis"])
    print(f"  {term['coefficient']:+.1f} * {ops}")
# +0.1 * Z0Z1, +0.3 * Z0, +0.2 * Z1, +0.5 * X0, +0.5 * X1
```
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Example 5: 16-qubit Heisenberg spin chain
# ═══════════════════════════════════════════════════════════════════════════════

spin_chain_terms = []
for i in range(15):
    spin_chain_terms.append(xx_term(i, i + 1, 1.0))
    spin_chain_terms.append(yy_term(i, i + 1, 1.0))
    spin_chain_terms.append(zz_term(i, i + 1, 1.0))

spin_chain_16q = {
    "version": 1,
    "system": {"n_qubits": 16},
    "hamiltonian": {
        "terms": spin_chain_terms,
        "constant_offset": 0.0,
    },
    "evolution": {
        "total_us": 0.5,
        "steps": 10,
        "dt_us": 0.05,
    },
    "observables": [
        {"type": "E"},
        {"type": "SZ", "qubit": 0},
    ],
    "noise": {
        "t1_us": 200.0,
        "t2_us": 150.0,
    },
}

SPIN_CHAIN_DOC = """\
# Example: Heisenberg Spin Chain, 16 Qubits

**File:** `spin_chain_16q.cbor.hex`
**Size:** 2515 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The isotropic Heisenberg XXX model on a 16-site linear chain:

```
H = sum_{i=0}^{14} (X_i X_{i+1} + Y_i Y_{i+1} + Z_i Z_{i+1})
```

15 nearest-neighbor pairs, 3 Pauli terms each = 45 total Pauli terms.

**Measure:** Energy (E) and sigma_z on qubit 0 (boundary magnetization)
**Evolution:** 0.5 us total, 10 Trotter steps (dt = 0.05 us)
**Noise floor:** T1 >= 200 us, T2 >= 150 us

## Physical Meaning

The Heisenberg spin chain is the canonical model of quantum magnetism. The
SU(2)-symmetric Hamiltonian preserves total spin, and its ground state exhibits
long-range antiferromagnetic correlations.

At 16 qubits, exact classical simulation requires diagonalizing a 65536x65536
matrix. This is at the boundary of classical tractability and represents a
natural target for quantum advantage in ground-state energy estimation.

The boundary magnetization <Z_0> reveals edge effects: in the
antiferromagnetic ground state of an open chain, boundary spins have enhanced
magnetization due to broken translational symmetry.

This example is designed for the Huoma quantum cloud -- it exercises the full
Trotterization and ZX-optimization pipeline at a scale that tests real
compiler performance.

## Parameters

| Parameter        | Value   | Unit     |
|------------------|---------|----------|
| Qubits           | 16      |          |
| Pauli terms      | 45      |          |
| XX pairs         | 15      |          |
| YY pairs         | 15      |          |
| ZZ pairs         | 15      |          |
| Coefficient      | 1.0     | GHz*rad  |
| Constant offset  | 0.0     | GHz*rad  |
| Total time       | 0.5     | us       |
| Trotter steps    | 10      |          |
| T1 minimum       | 200.0   | us       |
| T2 minimum       | 150.0   | us       |

## Expected Results

- Ground state energy: approximately -27.09 GHz*rad (Bethe ansatz)
- <Z_0> shows enhanced boundary magnetization
- The Trotter circuit has O(45 * 10) = 450 two-qubit gates before optimization

## Python Reconstruction

```python
import cbor2

with open("spin_chain_16q.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
print(f"Qubits: {program['system']['n_qubits']}")
print(f"Pauli terms: {len(program['hamiltonian']['terms'])}")
# 45 terms: 15 XX + 15 YY + 15 ZZ
```
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Example 6: H2 molecule (STO-3G, Jordan-Wigner)
# ═══════════════════════════════════════════════════════════════════════════════

h2_molecule = {
    "version": 1,
    "system": {"n_qubits": 2},
    "hamiltonian": {
        "terms": [
            # 0.3979 * IZ_1 -> 0.3979 * Z_1
            z_term(1, 0.3979),
            # -0.3979 * Z_0 I -> -0.3979 * Z_0
            z_term(0, -0.3979),
            # -0.0113 * Z_0 Z_1
            zz_term(0, 1, -0.0113),
            # 0.1809 * X_0 X_1
            xx_term(0, 1, 0.1809),
            # 0.1809 * Y_0 Y_1
            yy_term(0, 1, 0.1809),
        ],
        "constant_offset": -1.0523,
    },
    "evolution": {
        "total_us": 5.0,
        "steps": 20,
        "dt_us": 0.25,
    },
    "observables": [
        {"type": "E"},
    ],
    "noise": {
        "t1_us": 1000.0,
        "t2_us": 500.0,
    },
}

H2_DOC = """\
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
"""


# ═══════════════════════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════════════════════

def main() -> None:
    print("Generating new Ehrenfest CBOR examples...\n")

    examples = [
        ("qaoa_maxcut_4q", qaoa_maxcut_4q, QAOA_DOC),
        ("hubbard_2site", hubbard_2site, HUBBARD_DOC),
        ("ghz_8q", ghz_8q, GHZ_DOC),
        ("solvayeur_4backend", solvayeur_4backend, SOLVAYEUR_DOC),
        ("spin_chain_16q", spin_chain_16q, SPIN_CHAIN_DOC),
        ("h2_molecule", h2_molecule, H2_DOC),
    ]

    for name, program, doc in examples:
        print(f"[{name}]")
        write_example(name, program)
        write_doc(name, doc)
        print()

    print("Done. Verify with:")
    print("  for f in qaoa_maxcut_4q hubbard_2site ghz_8q solvayeur_4backend spin_chain_16q h2_molecule; do")
    print("    cat spec/examples/$f.cbor.hex | xxd -r -p > /tmp/$f.cbor")
    print("    cargo run -p afana -- /tmp/$f.cbor --qasm v3 --stats")
    print("  done")


if __name__ == "__main__":
    main()
