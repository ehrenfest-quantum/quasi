# Hubbard Model on 4-Site Square Lattice (8 Qubits)

This Ehrenfest program implements the Fermi-Hubbard model on a 2×2 square lattice with spin degrees of freedom, requiring 8 qubits (4 sites × 2 spins).

## Physics

The Hubbard model describes interacting fermions on a lattice with two competing terms:
- **Hopping (kinetic)**: Particles move between neighboring sites
- **On-site interaction**: Particles experience Coulomb repulsion when occupying the same site

For the 4-site square lattice with periodic boundary conditions, the Hamiltonian is:

H = -t Σ⟨ij⟩,σ (c†iσ cjσ + h.c.) + U Σi ni↑ ni↓

where:
- t: hopping amplitude (set to 1.0)
- U: on-site interaction (set to 4.0)
- σ ∈ {↑,↓}: spin index
- ⟨ij⟩: nearest-neighbor pairs

## Qubit Mapping

Jordan-Wigner transformation maps fermionic operators to Pauli matrices:
- Site 0: qubits 0 (spin-up), 1 (spin-down)
- Site 1: qubits 2 (spin-up), 3 (spin-down)
- Site 2: qubits 4 (spin-up), 5 (spin-down)
- Site 3: qubits 6 (spin-up), 7 (spin-down)

## Hamiltonian Terms

1. **Hopping terms** (XX + YY) for each spin on nearest-neighbor bonds:
   - Horizontal: (0,1), (2,3) for both spins
   - Vertical: (0,2), (1,3) for both spins
   - Coefficient: -t = -1.0

2. **Interaction terms** (ZZ) for each site:
   - Local ZZ between spin-up and spin-down qubits on same site
   - Coefficient: U/4 = 1.0 (after Jordan-Wigner)

## Parameters

- **Evolution time**: 8.0 μs
- **Trotter steps**: 10
- **Time step**: 0.8 μs

## Observables

Measures σᶻ on all 8 qubits to track particle density and spin configuration.

## Expected Results

For U/t = 4.0 at half-filling, the system exhibits:
- Antiferromagnetic correlations
- Mott insulating behavior
- Particle-hole symmetry in the density distribution

The exact ground state energy for this 4-site cluster at U=4t is approximately -4.0 (in units where t=1).