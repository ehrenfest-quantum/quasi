# Quantum Random Walk on a Cycle Graph (8 qubits)

This example models a continuous-time quantum random walk on a cycle graph with 8 qubits, where each qubit is connected to its two neighbors in a ring topology. The periodic boundary conditions introduce non-trivial spatial structure and are important for studying topological effects in closed quantum systems.

## Physics

The Hamiltonian consists of X⊗X interactions between adjacent qubits, with the final term connecting qubit 7 back to qubit 0 to complete the cycle:

```
H = Σ_{i=0}^7 (X_i X_{i+1 mod 8})
```

This represents a quantum walk where excitation can propagate around the ring. The system exhibits ballistic spreading and revivals due to the periodic boundary conditions.

## Parameters

- Number of qubits: 8
- Total evolution time: 5.0 μs
- Trotter steps: 10
- Timestep: 0.5 μs
- Required T1: 2000 μs
- Required T2: 1500 μs

## Observables

The program measures ⟨Z₀⟩, the expectation value of the Pauli-Z operator on the first qubit, which tracks the probability amplitude at the initial site.

## Expected Results

For a quantum walk starting at qubit 0, we expect oscillatory behavior in ⟨Z₀⟩ due to quantum interference and revivals from the periodic boundary conditions. The exact dynamics depend on the evolution time and graph symmetry.