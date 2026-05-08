# XXZ Spin Chain (8-qubit)

## Physics

The XXZ spin chain is a one-dimensional quantum spin model with anisotropic interactions. The Hamiltonian is:

$$H = J \sum_{i=0}^{N-2} (\sigma_i^x \sigma_{i+1}^x + \sigma_i^y \sigma_{i+1}^y + \Delta \sigma_i^z \sigma_{i+1}^z)$$

Where:
- $J$ is the coupling strength (set to 0.5)
- $\Delta$ is the anisotropy parameter (set to 0.25)
- $N$ is the number of qubits (8)

This model interpolates between the isotropic Heisenberg chain ($\Delta=1$) and the Ising chain ($\Delta=\infty$). For $\Delta<1$, the model is in the XY regime, and for $\Delta>1$, it's in the Ising regime.

## Parameters

- Number of qubits: 8
- Coupling strength (J): 0.5 GHz·rad
- Anisotropy parameter (Δ): 0.25
- Total evolution time: 1.0 μs
- Trotter steps: 10
- dt: 0.1 μs
- T1: 100 μs
- T2: 50 μs

## Expected Results

The Trotterized circuit will contain alternating layers of:
1. XX/YY interaction terms implemented with Hadamard rotations and CNOT ladders
2. ZZ interaction terms implemented with CNOT ladders and Rz rotations

The anisotropy parameter Δ will appear as a scaling factor in the ZZ interaction rotation angles. In the ZX-IR representation, this will manifest as phases on Z-spiders corresponding to the ZZ terms.

The circuit depth will be proportional to the number of Trotter steps (10) and the number of interaction terms (7 nearest-neighbor pairs).