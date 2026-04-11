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
