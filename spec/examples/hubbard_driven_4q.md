# Time-Dependent Driven Hubbard Model (4 Qubits)

A 4-site Hubbard model with time-dependent driving terms, representing interacting fermions on a lattice with external field modulation.

## Physics

The Hamiltonian implements:
- Hopping terms between adjacent sites
- On-site Coulomb repulsion
- Time-dependent driving field
- Spin degrees of freedom mapped to qubits

## Parameters

- **Sites**: 4 (2×2 lattice)
- **Total evolution time**: 60 μs
- **Trotter steps**: 100
- **Time step**: 0.6 μs
- **T1 requirement**: 100 μs
- **T2 requirement**: 50 μs

## Observables

- σᶻ on qubit 0 (site 0 spin-up)
- σᶻ on qubit 1 (site 0 spin-down)

## Expected Results

The time-dependent driving should induce coherent oscillations in the spin density at each site. The σᶻ measurements will show the time evolution of local magnetization, with characteristic frequencies determined by the hopping amplitude and driving strength.

## Implementation Notes

This model demonstrates Afana's capability to handle time-dependent Hamiltonians with external driving fields, essential for quantum simulation of condensed matter systems.