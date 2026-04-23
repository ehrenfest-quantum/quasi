# Quantum Phase Estimation for Molecular Hamiltonian (8 qubits)

This example demonstrates quantum phase estimation (QPE) applied to a molecular Hamiltonian with 8 qubits. The circuit implements controlled unitary operations for phase estimation, validating the full Afana compilation pipeline from Ehrenfest AST to QASM3 output.

## Physics

The Hamiltonian represents a simplified molecular system with nearest-neighbor interactions across 8 qubits. The phase estimation algorithm is used to find eigenvalues of the unitary operator U = exp(-iHt), where H is the molecular Hamiltonian.

## Parameters

- Number of qubits: 8
- Total evolution time: 100 μs
- Trotter steps: 10
- T1 requirement: 5000 μs
- T2 requirement: 4000 μs
- Observable: σᶻ on qubit 0

## Expected Results

The QASM3 output should contain:
- Controlled-U gates implementing the phase estimation
- Proper phase estimation circuit structure with ancilla qubits
- Valid time evolution under the molecular Hamiltonian
- Measurement of σᶻ on qubit 0

This example tests ZX-IR generation for controlled multi-qubit gates and phase estimation patterns, ensuring the compiler correctly handles complex quantum chemistry algorithms.