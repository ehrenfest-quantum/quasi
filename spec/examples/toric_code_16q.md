# Toric Code Stabilizer Hamiltonian (16 Qubits)

The toric code is a fundamental model in quantum error correction, representing a topological stabilizer code on a 2D lattice. This example demonstrates the encoding of a toric code Hamiltonian with plaquette and vertex stabilizers.

## Physics

The toric code Hamiltonian consists of two types of terms:
- Plaquette terms: Products of Z operators around each face (plaquette) of the square lattice.
- Vertex terms: Products of X operators at each vertex of the lattice.

The 16-qubit example uses a 4x4 grid with periodic boundary conditions, resulting in a highly connected qubit topology.

## Parameters

- **Qubits**: 16 (arranged on a 4x4 grid)
- **Plaquette terms**: Z operators around each face of the lattice
- **Vertex terms**: X operators at each vertex of the lattice
- **Evolution time**: 10 μs total, 1 step
- **Observables**: Energy expectation value

## Expected Results

The CBOR program encodes a toric code Hamiltonian with plaquette and vertex stabilizers. The expected ZX-IR structure will feature a highly connected graph with alternating Z and X spiders representing the stabilizer terms.
