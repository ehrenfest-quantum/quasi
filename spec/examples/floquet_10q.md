# Floquet Model: Time-Dependent Driven Hamiltonian (10 Qubits)

## Physics Overview

This example demonstrates a **Floquet model** describing a 10-qubit chain under a time-periodic driving field. The Hamiltonian consists of:

1. A time-independent longitudinal field (Ising-like ZZ interactions)
2. A time-dependent transverse field (X rotations) with sinusoidal modulation

The system exhibits rich dynamics including dynamical phase transitions and heating effects depending on the drive frequency and amplitude.

## Parameters

- **System size**: 10 qubits
- **Interaction**: Nearest-neighbor ZZ couplings (J=1.0 GHz)
- **Drive frequency**: Ω = 2π × 1 MHz
- **Drive amplitude**: A = 1.0 GHz
- **Total evolution time**: 100 μs
- **Trotter steps**: 20
- **Required T1**: 50 μs
- **Required T2**: 25 μs

## Expected Results

Under resonant driving conditions, the system shows periodic oscillations in local magnetization with frequency components at harmonics of the drive. Off-resonant driving leads to suppression of heating and stabilization of prethermal states.

## Hamiltonian

H(t) = Σᵢⱼ Jᵢⱼ Zᵢ Zⱼ + Σᵢ [A cos(Ωt)] Xᵢ

Where:
- Jᵢⱼ = 1.0 for nearest neighbors, 0 otherwise
- A = 1.0 GHz drive amplitude
- Ω = 2π × 1 MHz angular frequency

Observables include σᶻ expectation values for all 10 qubits to monitor magnetization dynamics.