# Quantum Random Walk on a 12-Qubit Cycle Graph

This example implements a discrete-time quantum random walk on a cycle graph with 12 qubits. Unlike the line graph (where boundary nodes have fewer neighbors) or the 2D grid (where corner and edge nodes have fewer neighbors), the cycle graph has uniform connectivity: each node has exactly two neighbors, forming a closed loop.

## Physics

The quantum random walk evolves under a Hamiltonian that encodes the adjacency structure of the cycle graph. For a cycle with 12 nodes, each node `i` is connected to nodes `(i-1) mod 12` and `(i+1) mod 12`. The Hamiltonian is:

```
H = -∑ᵢ (|i⟩⟨i+1| + |i+1⟩⟨i|)
```

In the Pauli basis, each hopping term `|i⟩⟨i+1| + |i+1⟩⟨i|` can be written as `XᵢXᵢ₊₁ + YᵢYᵢ₊₁`. The full Hamiltonian becomes:

```
H = -∑ᵢ (XᵢXᵢ₊₁ + YᵢYᵢ₊₁)
```

with periodic boundary conditions (qubit 11 connects back to qubit 0).

## Parameters

- **System**: 12 qubits arranged in a cycle
- **Evolution time**: 10.0 μs
- **Trotter steps**: 10
- **Time step**: 1.0 μs
- **Noise constraints**: T1 = 100 μs, T2 = 100 μs

## Observables

The example measures the expectation value of σᶻ on each qubit, which gives the probability of finding the walker at each position. For an initially localized state (e.g., qubit 0), the probability distribution spreads ballistically over time, with interference patterns arising from the quantum nature of the walk.

## Expected Results

After evolution, the probability distribution should show ballistic spreading with peaks near the initial position and its periodic images. The distribution will be symmetric due to the uniform structure of the cycle graph.

This example validates the compiler's ability to handle periodic boundary conditions in the Hamiltonian and correctly synthesize the resulting entangling gates into QASM3.