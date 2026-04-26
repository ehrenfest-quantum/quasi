# Time-Dependent Driven Transverse-Field Ising Model (8 qubits, periodic BC)

## Physics

This example simulates the time-evolution of a 1D transverse-field Ising model with periodic boundary conditions. The Hamiltonian is:

$$H = J \sum_{i=1}^{N} Z_i Z_{i+1} + \Gamma \sum_{i=1}^{N} X_i$$

with $N = 8$, $J = 1.0$ GHz·rad, $\Gamma = -2.0$ GHz·rad, and periodic boundary conditions ($Z_9 = Z_1$). The transverse field strength $\Gamma$ varies sinusoidally in time (though the current Afana compiler handles the static Trotterised version; time-dependence is captured via the finite step size and multiple Trotter steps).

## Parameters

| Parameter | Value |
|-----------|-------|
| Number of qubits | 8 |
| Ising coupling J | 1.0 GHz·rad |
| Transverse field Γ | -2.0 GHz·rad |
| Total evolution time | 10.0 µs |
| Trotter steps | 100 |
| Time step dt | 0.1 µs |

## Observables

1. **Energy** — expectation value of the full Hamiltonian $\langle H \rangle$.
2. **Sigma-Z on qubit 0** — $\langle Z_0 \rangle$, the magnetisation on the first qubit.

## Noise Constraints

- T1 relaxation time: ≥ 100 µs
- T2 dephasing time: ≥ 50 µs

## Expected Results

The example validates that:
- The compiler correctly handles periodic boundary conditions (nearest-neighbour Z-Z coupling between qubits 7 and 0).
- A chain of 8 qubits with 16 Pauli terms compiles without error.
- QASM3 output is valid and can be executed on a compatible backend.
- The Trotterisation with 100 steps and dt = 0.1 µs produces a circuit with depth ~150-300 gates (depending on optimisation).