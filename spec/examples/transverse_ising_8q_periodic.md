# Transverse-Field Ising Model (8 Qubits, Periodic Boundary Conditions)

## Physics Overview

The transverse-field Ising model (TFIM) is a paradigmatic quantum many-body system used extensively in quantum simulation, benchmarking, and the study of quantum phase transitions.

### Hamiltonian

$$H = -J \sum_{i=0}^{N-1} Z_i Z_{i+1} - h \sum_{i=0}^{N-1} X_i$$

where:
- $N = 8$ qubits
- $J = 1.0$ GHz·rad (interaction strength)
- $h = 0.5$ GHz·rad (transverse field strength)
- **Periodic boundary conditions**: $Z_7 Z_0$ term included (qubit 7 couples back to qubit 0)

### Pauli Term Structure

The Hamiltonian contains 16 terms:
- **8 ZZ interaction terms**: $Z_0Z_1, Z_1Z_2, Z_2Z_3, Z_3Z_4, Z_4Z_5, Z_5Z_6, Z_6Z_7, Z_7Z_0$
- **8 X transverse field terms**: $X_0, X_1, X_2, X_3, X_4, X_5, X_6, X_7$

### Significance of Periodic Boundary Conditions

Periodic boundary conditions (PBC) introduce a ring topology where the first and last qubits are connected. This:
1. Eliminates edge effects present in open chains
2. Preserves translational symmetry
3. Enables study of bulk properties in finite systems
4. Increases circuit complexity (additional entangling gate required)

## Example Parameters

| Parameter | Value | Unit |
|-----------|-------|------|
| Qubits | 8 | - |
| Interaction strength (J) | 1.0 | GHz·rad |
| Transverse field (h) | 0.5 | GHz·rad |
| Total evolution time | 1.0 | μs |
| Trotter steps | 10 | - |
| Timestep (dt) | 0.1 | μs |
| Minimum T1 | 1.0 | μs |
| Minimum T2 | 1.0 | μs |

## Expected Results

When compiled and executed:
- **ZX-IR lowering**: 16 Hamiltonian terms → Trotterized gate sequence
- **Entangling gates**: 8 CZ gates per Trotter step (one for each ZZ term)
- **Single-qubit gates**: 8 RX gates per Trotter step (for X terms)
- **Circuit depth**: Scales linearly with Trotter steps

## Usage

```bash
# Compile to QASM3
cat spec/examples/transverse_ising_8q_periodic.cbor.hex | xxd -r -p > /tmp/tfim8.cbor
./target/release/afana /tmp/tfim8.cbor --qasm v3 --stats

# With optimization
./target/release/afana /tmp/tfim8.cbor --qasm v3 --optimize --stats
```

## References

1. Sachdev, S. *Quantum Phase Transitions*. Cambridge University Press, 2011.
2. Pfeuty, P. "The one-dimensional Ising model with a transverse field." *Annals of Physics* 57.1 (1970): 79-90.
3. Ehrenfest specification v0.1 — Hamiltonian encoding for quantum simulation.
