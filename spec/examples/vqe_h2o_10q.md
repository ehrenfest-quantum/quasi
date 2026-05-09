# Variational Quantum Eigensolver Ansatz for H₂O Molecule (10 qubits)

## Physics

This example implements a Variational Quantum Eigensolver (VQE) ansatz for computing the ground state energy of the water molecule using the Jordan-Wigner mapping. The molecular geometry is optimized at the RHF/STO-3G level of theory.

The Hamiltonian is expressed as a sum of Pauli tensor products derived from second-quantized fermionic operators mapped to qubit operators. The ansatz uses a hardware-efficient approach with alternating layers of single-qubit rotations and entangling gates.

## Parameters

- **Qubits**: 10 (Jordan-Wigner mapped from 10 spin-orbitals)
- **Evolution time**: 1.0 μs
- **Trotter steps**: 1000
- **T1 time requirement**: 100 μs
- **T2 time requirement**: 50 μs
- **Minimum gate fidelity**: 0.99

## Expected Results

- Ground state energy: approximately -74.989 Hartree
- Convergence within 100 variational iterations
- Circuit depth suitable for NISQ devices with error mitigation

The ansatz prepares a trial wavefunction |ψ(θ)⟩ parameterized by rotational angles θ. These angles are optimized classically to minimize the expectation value ⟨ψ(θ)|H|ψ(θ)⟩.