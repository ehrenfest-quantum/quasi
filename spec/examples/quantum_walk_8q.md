# Quantum Random Walk on a Line Graph (8 Qubits)

## Physics Description
This example simulates a discrete-time quantum random walk on a 1D line graph consisting of 8 nodes. The Hamiltonian is constructed to represent the adjacency matrix of the line graph, facilitating coherent transport across the lattice.

## Parameters
- **Number of Qubits**: 8
- **Graph Topology**: Line graph (nodes connected linearly: 0-1-2-3-4-5-6-7)
- **Evolution Time**: 4.0 $\mu$s
- **Trotter Steps**: 100
- **Timestep ($\Delta t$)**: 0.04 $\mu$s

## Expected Results
- The walker's probability distribution should exhibit characteristic quantum interference patterns, spreading faster than a classical random walk.
- Observables: $\langle Z_0 \rangle$ and $\langle Z_7 \rangle$ to monitor the presence of the walker at the boundaries.