# Floquet Hamiltonian with Time-Dependent Driving (8 Qubits)

## Physics

This example implements a Floquet system with periodic driving on 8 qubits. Floquet theory describes quantum systems under periodic time-dependent Hamiltonians H(t) = H(t + T), where T is the driving period.

The Hamiltonian consists of:
- **Static ZZ interactions**: Nearest-neighbor coupling along a chain
  ```
  H_static = Sum_{i=0}^{6} Z_i Z_{i+1}
  ```
- **Time-dependent X driving**: Global periodic drive (represented as static X terms for Trotterization)
  ```
  H_drive = Omega * Sum_{i=0}^{7} X_i
  ```

The full Hamiltonian: H = H_static + H_drive

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 8 | Chain of 8 qubits |
| ZZ coupling | 1.0 GHz·rad | Nearest-neighbor interaction strength |
| X drive (Omega) | 0.5 GHz·rad | Driving field amplitude |
| total_us | 1000.0 | Total evolution time (μs) |
| steps | 8 | Trotter steps |
| dt_us | 125.0 | Timestep (μs) |
| t1_us | 50000 | T1 relaxation time (μs) |
| t2_us | 30000 | T2 dephasing time (μs) |

## Observables

- **Energy**: Expectation value ⟨H⟩ of the full Hamiltonian
- **Sigma-Z on qubit 0**: Local magnetization ⟨Z₀⟩

## Expected Results

- ZX-IR generation with 8 qubits
- Trotterized gate sequence with ZZ and X gates
- Circuit depth within T2 budget (30,000 μs)
- Valid QASM3 output with proper parameter binding

## Use Case

This example validates the Afana compiler's handling of:
1. Multi-qubit Hamiltonians (8+ qubits per charter benchmark scale)
2. Mixed interaction types (ZZ + X)
3. Floquet-style periodic driving representation
4. ZX-IR lowering for time-dependent systems

## Compilation

```bash
# Compile to QASM3
afana spec/examples/floquet_8q.cbor.hex --qasm v3

# With optimization
afana spec/examples/floquet_8q.cbor.hex --qasm v3 --optimize --stats
```