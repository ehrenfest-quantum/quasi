# Floquet Hamiltonian with Time-Dependent Driving (8 qubits)

## Physics

This example models a driven 8-qubit system with time-periodic Hamiltonian:

H(t) = ∑ᵢⱼ Jᵢⱼ σᶻᵢ σᶻⱼ + ∑ᵢ hᵢ(t) σᶻᵢ + Γ ∑ᵢ σˣᵢ

Where:
- Jᵢⱼ are nearest-neighbor ZZ couplings
- hᵢ(t) = h₀ cos(ωt + φᵢ) are time-dependent local fields
- Γ is the transverse field strength

## Parameters

- **n_qubits**: 8
- **total_time**: 25.0 μs
- **trotter_steps**: 10
- **dt**: 2.5 μs
- **J**: 0.1 GHz
- **h₀**: 0.05 GHz
- **ω**: 2π × 1 GHz
- **Γ**: 0.02 GHz
- **T1_min**: 50 μs
- **T2_min**: 25 μs

## Expected Results

The system exhibits Floquet dynamics with quasi-energy bands. The time-averaged magnetization ⟨σᶻ⟩ should show oscillations at the drive frequency, with amplitude dependent on the ratio Γ/h₀.