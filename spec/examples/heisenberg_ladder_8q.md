# Heisenberg Model on 2D Ladder (8 Qubits)

## Physics

This example encodes the isotropic Heisenberg (XXX) model on a 2×4 ladder geometry with 8 qubits. The Hamiltonian describes nearest-neighbor spin-spin interactions:

$$H = J \sum_{\langle i,j \rangle} (X_i X_j + Y_i Y_j + Z_i Z_j)$$

where $J = 1.0$ GHz·rad is the coupling strength and $\langle i,j \rangle$ denotes nearest-neighbor pairs.

## Geometry

Qubit layout (ladder topology):
```
0 ── 1 ── 2 ── 3
│    │    │    │
4 ── 5 ── 6 ── 7
```

**Edges (10 total):**
- Horizontal top: (0,1), (1,2), (2,3)
- Horizontal bottom: (4,5), (5,6), (6,7)
- Vertical rungs: (0,4), (1,5), (2,6), (3,7)

**Hamiltonian terms:** 30 Pauli terms (3 per edge: XX, YY, ZZ)

## Parameters

| Parameter | Value |
|-----------|-------|
| Qubits | 8 |
| Coupling J | 1.0 GHz·rad |
| Total evolution time | 10.0 μs |
| Trotter steps | 10 |
| Timestep dt | 1.0 μs |
| T1 requirement | 100.0 μs |
| T2 requirement | 50.0 μs |

## Observables

1. **Energy (E)**: Expectation value ⟨H⟩ of the full Hamiltonian
2. **Sigma-Z on qubit 0 (SZ)**: Local magnetization ⟨Z₀⟩

## Expected Results

- Ground state energy: Approximately -10.0 GHz·rad (10 edges × -1.0 per edge for antiferromagnetic ordering)
- The ladder geometry introduces non-trivial quantum correlations beyond a simple 1D chain
- Trotterization error scales as O(dt²) for first-order Trotter

## Compilation

```bash
# Decode CBOR hex to binary
cat spec/examples/heisenberg_ladder_8q.cbor.hex | xxd -r -p > /tmp/heisenberg_ladder_8q.cbor

# Compile to QASM3
./target/release/afana /tmp/heisenberg_ladder_8q.cbor --qasm v3 --stats

# Expected: ~300 gates (30 terms × 10 steps × ~1 gate per Pauli rotation)
```

## Validation

```rust
use afana::cbor::from_cbor;
use std::fs;

let hex = fs::read_to_string("spec/examples/heisenberg_ladder_8q.cbor.hex").unwrap();
let bytes = hex::decode(hex.trim()).unwrap();
let program = from_cbor(&bytes).unwrap();

assert_eq!(program.system.n_qubits, 8);
assert_eq!(program.hamiltonian.terms.len(), 30);
assert_eq!(program.evolution.steps, 10);
```
