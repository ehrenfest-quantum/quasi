# Floquet Driven 8‑Qubit Example

This Ehrenfest example demonstrates a simple Floquet‑driven system on **8 qubits**. The Hamiltonian consists of a static term `X₀` with coefficient 1.0 and a constant offset of 0.0. The system evolves for a total of **1000 µs** using **10 Trotter steps** (Δt = 100 µs). The observable measured is the **energy** of the Hamiltonian.

Noise constraints require a minimum **T₁ = 1000 µs** and **T₂ = 500 µs**, ensuring the program type‑checks and can be compiled by Afana.

Running the example:
```bash
cat spec/examples/floquet_driven_8q.cbor.hex | xxd -r -p > /tmp/floquet.cbor
./target/release/afana /tmp/floquet.cbor --qasm v3
```
The output should be valid OpenQASM 3 that passes the `qasm3-validator`.
