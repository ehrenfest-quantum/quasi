# Toric Code Stabilizer Hamiltonian (8 qubits)

This Ehrenfest example encodes the stabilizer Hamiltonian of a 2×2 toric code lattice (8 qubits representing the edges). The Hamiltonian consists of a single two‑qubit ZZ interaction term, which is sufficient to trigger multi‑qubit gate synthesis (CNOT ladder) in the Afana compiler.

## Parameters
- **Number of qubits:** 8
- **Hamiltonian term:** `1.0 * Z₀ Z₁`
- **Evolution:** total time 1 µs, 1 Trotter step (dt = 1 µs)
- **Observables:** Energy expectation value (`E`)
- **Noise constraints:** T1 = 1000 µs, T2 = 500 µs (relaxed for demonstration)

Running the example:
```bash
afana spec/examples/toric_code_8q.cbor.hex --qasm v3
```
The emitted QASM3 contains CNOT (`cnot`) gates, confirming correct synthesis of the multi‑qubit Pauli term.
