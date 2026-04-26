# Heisenberg 2D Grid (4 qubits, 2×2)

## Physics

This example implements the Heisenberg model on a 2×2 square lattice with nearest-neighbor XX+YY+ZZ interactions. The Hamiltonian is:

```
H = J Σ_{⟨i,j⟩} (X_i X_j + Y_i Y_j + Z_i Z_j)
```

where J = 1.0 (ferromagnetic coupling) and ⟨i,j⟩ runs over nearest-neighbor pairs on the 2×2 grid. The grid has 4 edges: (0,1), (0,2), (1,3), (2,3).

## Parameters

- **Qubits**: 4 (arranged as 2×2 grid)
- **Coupling**: J = 1.0 GHz·rad
- **Evolution**: total = 1.0 μs, steps = 5, dt = 0.2 μs
- **Observable**: ⟨Z₀⟩ (sigma-Z expectation on qubit 0)
- **Noise**: T1 = 100 μs, T2 = 50 μs

## Expected results

When compiled with `--qasm v3`, the emitted QASM3 should contain:
- At least one `cx` gate (from the CNOT ladders used to implement XX, YY, ZZ terms)
- At least one `rz` gate (from the rotation angle in each Pauli term decomposition)
- Valid OpenQASM 3.0 syntax passing `oq3` validation

The Trotterized circuit will have 5 Trotter steps, each applying the 4 nearest-neighbor terms in first-order decomposition. Each two-qubit Pauli term (XX, YY, ZZ) decomposes into a CNOT ladder with an Rz rotation, producing the required gate types.
