# Grover Search Algorithm (4-Qubit Database)

This Ehrenfest program implements Grover's search algorithm for a 4-qubit database (N=16 items). Grover's algorithm provides a quadratic speedup for unstructured search problems, requiring O(√N) queries instead of O(N) classical queries.

## Physics

The Hamiltonian models the oracle function and diffusion operator of Grover's algorithm:

1. **Oracle Term**: Encodes the marked item (assumed to be |1111⟩) with a phase flip
2. **Diffusion Operator**: Implements the inversion about the mean operation
3. **Combined Evolution**: Alternates between oracle and diffusion steps

## Parameters

- **n_qubits**: 4 (2 for database index, 2 for ancilla)
- **total_us**: 8.0 (evolution time)
- **steps**: 1 (single Grover iteration)
- **dt_us**: 8.0 (timestep)
- **t1_us**: 100.0 (T1 relaxation time constraint)
- **t2_us**: 100.0 (T2 dephasing time constraint)

## Expected Results

When measured in the computational basis, the system should show high probability amplitude at the marked state |1111⟩ after evolution. The expectation values ⟨σᶻ⟩ for each qubit should approach +1 for the marked state, indicating the solution has been found with high probability.

## Gate Synthesis

Afana's Trotterization and ZX-IR lowering will decompose this Hamiltonian into elementary gates including Hadamard gates for superposition preparation, controlled-Z gates for the oracle implementation, and multi-qubit rotations for the diffusion operator.