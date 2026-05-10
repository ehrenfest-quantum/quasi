# XXZ Spin Chain (12 Qubits)

## Physics Description

The XXZ spin chain is a one-dimensional quantum spin model with anisotropic interactions, described by the Hamiltonian:

$$H = J \sum_{i=0}^{N-2} (\sigma_i^x \sigma_{i+1}^x + \sigma_i^y \sigma_{i+1}^y + \Delta \sigma_i^z \sigma_{i+1}^z)$$

Where:
- $J$ is the coupling strength (set to 1.0)
- $\Delta$ is the anisotropy parameter (set to 1.0 for the Heisenberg point)
- $N$ is the number of qubits (12 in this example)

This model exhibits rich physics including quantum phase transitions as the anisotropy parameter $\Delta$ is varied:
- $\Delta < -1$: Ferromagnetic Ising phase
- $-1 < \Delta < 1$: XY phase with algebraic decay of correlations
- $\Delta > 1$: Antiferromagnetic Ising phase

## Parameters

- **Qubits**: 12
- **Coupling strength (J)**: 1.0 GHz·rad
- **Anisotropy parameter (Δ)**: 1.0 (Heisenberg point)
- **Evolution time**: 10.0 μs
- **Trotter steps**: 1
- **Required T1**: 100.0 μs
- **Required T2**: 100.0 μs

## Expected Results

At the Heisenberg point ($\Delta = 1$), this model exhibits SU(2) symmetry and is expected to show antiferromagnetic correlations. The ground state has total spin $S_{total} = 0$ for even $N$, with alternating spin expectation values along the z-axis.

The energy per bond is expected to approach $E_0/N \approx -0.443$ GHz·rad in the thermodynamic limit, with finite-size corrections for the 12-qubit chain.

## Circuit Properties

- **Depth**: ~50 gates
- **CX gates**: 22 (nearest-neighbor)
- **Single-qubit gates**: 24 (hadamard and phase)
- **Qubit connectivity**: Linear chain (0-1-2-...-11)