# Heisenberg Model on 2D Square Grid (4 Qubits)

## Physics Background

The Heisenberg model is a fundamental quantum many-body system describing interacting spins on a lattice. The Hamiltonian is:

$$H = J \sum_{\langle i,j \rangle} (X_i X_j + Y_i Y_j + Z_i Z_j)$$

where $\langle i,j \rangle$ denotes nearest-neighbor pairs and $J$ is the coupling strength.

## System Configuration

- **Qubits**: 4 qubits arranged in a 2D square grid
- **Connectivity**:
  ```
  0 -- 1
  |    |
  2 -- 3
  ```
- **Nearest-neighbor pairs**: (0,1), (0,2), (1,3), (2,3)
- **Coupling**: J = 1.0 (antiferromagnetic)
- **Hamiltonian terms**: 12 total (4 XX + 4 YY + 4 ZZ interactions)

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 4 | Number of qubits in the system |
| J | 1.0 GHz·rad | Coupling strength |
| total_us | 10.0 | Total evolution time (μs) |
| steps | 10 | Number of Trotter steps |
| dt_us | 1.0 | Time step (μs) |
| t1_us | 100.0 | Minimum T1 relaxation time (μs) |
| t2_us | 50.0 | Minimum T2 dephasing time (μs) |

## Observables

The program measures:
1. **Energy** (E): Expectation value ⟨H⟩ of the full Hamiltonian
2. **⟨Z₀⟩**: Sigma-Z on qubit 0
3. **⟨Z₁⟩**: Sigma-Z on qubit 1
4. **⟨Z₂⟩**: Sigma-Z on qubit 2
5. **⟨Z₃⟩**: Sigma-Z on qubit 3

## Expected Results

For the ground state of the antiferromagnetic Heisenberg model on a 2×2 grid:

- **Ground state energy**: Approximately -4.0 GHz·rad (for J=1)
- **Magnetization**: ⟨Zᵢ⟩ ≈ 0 for all qubits (no net magnetization in ground state)
- **Symmetry**: The system has SU(2) symmetry, so ⟨X⟩ = ⟨Y⟩ = ⟨Z⟩ = 0 in the ground state

## Compilation Notes

- Afana will Trotterize the Hamiltonian into gate sequences
- Each Trotter step decomposes the 12 Pauli terms into native gates
- ZX-calculus optimization reduces gate count by identifying cancellations
- Expected circuit depth: ~100-200 gates depending on optimization level

## Usage

```bash
# Compile to QASM3
cat spec/examples/heisenberg_2d_4q.cbor.hex | xxd -r -p > /tmp/h2d4.cbor
./target/release/afana /tmp/h2d4.cbor --qasm v3 --optimize --stats

# View statistics (gate counts, circuit depth, noise analysis)
./target/release/afana /tmp/h2d4.cbor --stats
```

## References

1. Heisenberg, W. (1928). "Zur Theorie des Ferromagnetismus". Zeitschrift für Physik.
2. Sandvik, A. W. (2010). "Computational Studies of Quantum Spin Systems". AIP Conference Proceedings.
3. Ehrenfest specification: spec/ehrenfest-v0.1.cddl
