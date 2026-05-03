# Quantum Random Walk on a Line Graph (10 qubits)

## Physics

A continuous-time quantum random walk on a line graph with 10 vertices. The walker hops between adjacent sites with amplitude -1 (hopping Hamiltonian). This models quantum transport and diffusion processes.

## Hamiltonian

H = -Σᵢ (Xᵢ Xᵢ₊₁ + Yᵢ Yᵢ₊₁) for i = 0..8

Nearest-neighbor hopping terms couple adjacent qubits with strength -1 GHz·rad. The negative sign ensures the ground state corresponds to constructive interference across the chain.

## Parameters

- Qubits: 10
- Evolution time: 5 µs
- Trotter steps: 50
- Time step: 0.1 µs

## Observables

- Sigma-Z on qubit 0: measures population at the leftmost site

## Noise Requirements

- T₁ ≥ 500 µs
- T₂ ≥ 250 µs

These constraints ensure phase coherence is maintained across the 5 µs evolution, typical for modern superconducting transmon devices.

## Expected Results

The walker initially localized at site 0 will spread ballistically, showing characteristic quantum interference patterns. The sigma-Z measurement on qubit 0 will oscillate as probability amplitude returns to the starting site.