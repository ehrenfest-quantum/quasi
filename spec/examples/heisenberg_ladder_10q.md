# Heisenberg Model on 2D Ladder (10 qubits)

## Physics Model

This example implements the antiferromagnetic Heisenberg model on a 2D ladder geometry with 10 qubits arranged as a 2×5 grid:

```
Qubit layout (ladder geometry):
  0 —— 1 —— 2 —— 3 —— 4
  |    |    |    |    |
  5 —— 6 —— 7 —— 8 —— 9
```

The Hamiltonian is:

```
H = J × Σ⟨i,j⟩ (X_i X_j + Y_i Y_j + Z_i Z_j)
```

where J = 1.0 GHz is the coupling strength and ⟨i,j⟩ denotes nearest-neighbor bonds.

### Bond structure (13 bonds total):
- **Horizontal bonds (8)**: (0,1), (1,2), (2,3), (3,4), (5,6), (6,7), (7,8), (8,9)
- **Vertical rungs (5)**: (0,5), (1,6), (2,7), (3,8), (4,9)

Each bond contributes 3 Pauli terms (XX, YY, ZZ), giving 39 total Pauli terms.

## Parameters

| Parameter | Value |
|-----------|-------|
| Qubits | 10 |
| Coupling J | 1.0 GHz |
| Total evolution time | 1.0 μs |
| Trotter steps | 10 |
| Time step dt | 0.1 μs |
| T1 constraint | 30 μs |
| T2 constraint | 15 μs |

## Observables

- **Energy**: ⟨H⟩ expectation value of the full Hamiltonian
- **Local magnetization**: ⟨Z₀⟩ on qubit 0

## Expected Results

- **Ground state**: Antiferromagnetic ordering with staggered magnetization
- **Energy**: For 10-qubit ladder with J=1.0, ground state energy ≈ -13.0 GHz (13 bonds × -1.0 per bond in ideal AFM)
- **Circuit depth**: ~400-600 gates after Trotterization and compilation (depends on optimization)
- **Fidelity estimate**: >90% on hardware with T2 > 15 μs for this circuit depth

## Compilation

```bash
# Compile to QASM3
cat spec/examples/heisenberg_ladder_10q.cbor.hex | xxd -r -p > /tmp/hl10.cbor
./target/release/afana /tmp/hl10.cbor --qasm v3 --optimize --stats
```

## Use Cases

- Benchmark for ZX-calculus optimization on medium-depth circuits
- Testing hardware-aware compilation on ladder topology
- Studying entanglement growth in 2D geometries
- Compiler stress test: 39 Pauli terms, 10 qubits, multi-step Trotterization

## References

- Heisenberg, W. (1928). "Zur Theorie des Ferromagnetismus". Z. Phys. 49: 619.
- Ehrenfest program specification: spec/ehrenfest-v0.1.cddl
