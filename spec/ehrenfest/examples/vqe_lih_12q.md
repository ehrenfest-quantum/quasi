# VQE LiH Molecule (12 qubits)

## Physics Overview

This Ehrenfest program encodes the Lithium Hydride (LiH) molecule in the 6-31G basis set, mapped to 12 qubits via Jordan-Wigner transformation. LiH is a benchmark system in quantum chemistry for variational quantum eigensolver (VQE) algorithms.

## Hamiltonian Structure

The molecular Hamiltonian is expressed as a sum of Pauli terms:

```
H = Σᵢ cᵢ Pᵢ + E₀
```

where Pᵢ are tensor products of Pauli operators and cᵢ are coefficients derived from electronic structure calculations.

### Included Pauli Terms

The example includes representative terms:
- XX interactions (qubits 0-1)
- ZZ interactions (qubits 0-1, 4-5)
- Single-qubit Z terms (qubits 0, 1)
- XY interactions (qubits 2-3)

Coefficients are scaled for demonstration purposes.

## Parameters

| Parameter | Value |
|-----------|-------|
| Qubits | 12 |
| Total evolution time | 100 μs |
| Trotter steps | 10 |
| Time step (dt) | 10 μs |
| T1 constraint | 100 μs |
| T2 constraint | 50 μs |

## Observables

1. **Energy (E)**: Expectation value of the full Hamiltonian ⟨H⟩
2. **Sigma-Z on qubit 0**: ⟨Z₀⟩

## Expected Results

When compiled with Afana:
- QASM3 output contains parametric rotation gates (Rz, Ry) for VQE ansatz
- Pauli evolution blocks from Trotterization
- Circuit depth suitable for NISQ devices

## Usage

```bash
# Compile to QASM3
cat spec/ehrenfest/examples/vqe_lih_12q.cbor.hex | xxd -r -p > /tmp/vqe_lih.cbor
./target/release/afana /tmp/vqe_lih.cbor --qasm v3 --stats
```

## References

- See et al., "Scalable Quantum Simulation of Molecular Energies", Phys. Rev. X (2016)
- Kandala et al., "Hardware-efficient VQE on a superconducting quantum processor", Nature (2017)
