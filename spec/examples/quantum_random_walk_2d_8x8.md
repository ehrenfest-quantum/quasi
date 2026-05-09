# Quantum Random Walk on 2D Grid (8x8)

## Physics

This example implements a continuous-time quantum random walk on a 2D square grid with 8x8 = 64 sites. The Hamiltonian uses XX + YY couplings between adjacent sites, equivalent to a tight-binding model for a single particle hopping on a 2D lattice.

## Hamiltonian

```
H = 0.5 * Σ_<i,j> (X_i X_j + Y_i Y_j)
```

where the sum runs over all horizontally and vertically adjacent pairs of qubits.

- **Horizontal edges**: 8 rows × 7 edges = 56 edges → 112 Pauli terms (XX + YY each)
- **Vertical edges**: 7 rows × 8 columns = 56 edges → 112 Pauli terms
- **Total terms**: 224

## Parameters

| Parameter | Value |
|-----------|-------|
| Qubits | 64 (8×8 grid) |
| Total evolution time | 100 μs |
| Trotter steps | 10 |
| Time step (dt) | 10 μs |
| Coupling strength | 0.5 GHz·rad |
| T1 requirement | 100 μs |
| T2 requirement | 50 μs |

## Observables

- σᶻ expectation on qubit 0 (corner site)

## Expected Results

- ZX-IR graph with 64 input spiders, 64 output spiders, and many internal spiders from the entangling gates
- Circuit depth proportional to the number of Trotter steps × Hamiltonian terms
- Non-trivial entanglement structure reflecting the 2D grid topology

## Usage

```bash
# Compile to QASM3 with ZX optimization
cat spec/examples/quantum_random_walk_2d_8x8.cbor.hex | xxd -r -p > /tmp/qrw.cbor
./target/release/afana /tmp/qrw.cbor --qasm v3 --optimize --stats
```
