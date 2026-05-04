# QAOA for 3-Colorable Graph (8 Qubits)

This example implements the Quantum Approximate Optimization Algorithm (QAOA) for a 3-colorable graph problem using 8 qubits. The problem involves assigning one of three colors to each node in a graph such that no two connected nodes share the same color.

## Physics

The Hamiltonian encodes the graph coloring constraints:
- Each node is represented by two qubits (enabling 4 states, but we use only 3 for the three colors)
- Penalty terms enforce that adjacent nodes have different colors
- The cost function is minimized when all constraints are satisfied

## Parameters

- Number of qubits: 8 (representing 4 nodes with 2 qubits each)
- Evolution time: 20 μs
- Trotter steps: 10
- Required T1: 50 μs
- Required T2: 50 μs

## Expected Results

When properly optimized, this circuit should find valid 3-colorings of the graph with high probability. The energy minimum corresponds to configurations where no adjacent nodes share the same color.