# VQE Ansatz for H₂ Molecule (8 qubits)

## Physics

This program implements a variational quantum eigensolver (VQE) ansatz for the hydrogen molecule (H₂) in the STO-3G minimal basis set. The system is mapped to 8 qubits using the Jordan-Wigner transformation. The Hamiltonian includes one-body and two-body terms from the molecular electronic structure, with a constant offset of -0.565 Hartree.

## Parameters

- **Molecule**: H₂
- **Basis set**: STO-3G
- **Qubits**: 8
- **Hamiltonian terms**: 1 Pauli term (coefficient -0.124)
- **Constant offset**: -0.565 Hartree
- **Evolution**: single Trotter step

## Expected Results

The ground state energy of H₂ in STO-3G basis is approximately -1.1373 Hartree. After subtracting the constant offset, the variational ansatz should converge to this value with appropriate parameter optimization.

## Usage

```bash
# Compile to QASM3
cargo run -p afana --release -- spec/examples/vqe_h2_8q.cbor.hex --qasm v3

# With optimization
cargo run -p afana --release -- spec/examples/vqe_h2_8q.cbor.hex --qasm v3 --optimize
```
