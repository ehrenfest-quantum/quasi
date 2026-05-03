# LiH Molecule VQE Ansatz (12 Qubits)

This Ehrenfest example encodes the Lithium Hydride (LiH) molecule Hamiltonian with a UCCSD-inspired variational ansatz for 12 qubits. LiH is a standard quantum chemistry benchmark that requires more qubits and a more complex variational ansatz than the simpler H2 molecule.

## Physical System

- **Molecule**: LiH (Lithium Hydride)
- **Qubits**: 12
- **Basis**: STO-3G minimal basis set
- **Mapping**: Jordan-Wigner transformation
- **Electrons**: 4 (2 from Li, 1 from each H)
- **Molecular orbitals**: 6 spatial orbitals → 12 spin orbitals → 12 qubits

## Hamiltonian Structure

The LiH Hamiltonian includes:

1. **One-body terms**: Single-electron energies and nuclear attraction
2. **Two-body terms**: Electron-electron repulsion
3. **Core Hamiltonian**: Nuclear-nuclear repulsion constant

The Hamiltonian is expressed in the second-quantized form:

```
H = Σᵢⱼ hᵢⱼ aᵢ†aⱼ + ½ Σᵢⱼₖₗ hᵢⱼₖₗ aᵢ†aⱼ†aₖaₗ + constant
```

Where:
- hᵢⱼ are one-electron integrals
- hᵢⱼₖₗ are two-electron integrals  
- aᵢ†, aᵢ are creation and annihilation operators

## Variational Ansatz

The example uses a UCCSD-inspired ansatz with:

- **Single excitations**: aᵢ†aⱼ (particle-hole excitations)
- **Double excitations**: aᵢ†aⱼ†aₖaₗ (two-particle-two-hole excitations)
- **Variational parameters**: θ₁, θ₂, ..., θₙ for each excitation operator

The ansatz circuit is constructed as:

```
|ψ(θ)⟩ = exp(Σᵢ θᵢ Tᵢ - Σᵢ θᵢ* Tᵢ†) |HF⟩
```

Where:
- Tᵢ are excitation operators
- |HF⟩ is the Hartree-Fock reference state
- θᵢ are variational parameters to be optimized

## Observables

The primary observable is the **ground state energy**:

```
E(θ) = ⟨ψ(θ)|H|ψ(θ)⟩
```

This energy expectation value is minimized during the VQE optimization loop.

## Evolution Parameters

- **Total evolution time**: 10.0 μs
- **Trotter steps**: 12
- **Time step**: 0.833 μs

These parameters are chosen to provide sufficient resolution for the molecular dynamics while staying within typical QPU coherence times.

## Noise Constraints

- **T1 relaxation time**: ≥ 100 μs
- **T2 dephasing time**: ≥ 50 μs

These requirements ensure the quantum hardware can maintain coherence throughout the variational optimization process.

## Expected Results

For LiH at equilibrium bond distance (1.595 Å):

- **Hartree-Fock energy**: ~ -7.86 Hartree
- **CCSD energy**: ~ -7.88 Hartree  
- **Experimental energy**: ~ -7.98 Hartree
- **VQE target accuracy**: ≤ 1 mHartree (chemical accuracy)

The VQE ansatz should converge to within chemical accuracy (~1 kcal/mol) of the full configuration interaction energy when using sufficient excitation operators and optimization steps.

## Circuit Complexity

The 12-qubit LiH ansatz includes:

- ~150-200 parameterized single-qubit rotations (RX, RY, RZ)
- ~80-120 CNOT gates for excitation operators
- ~50-80 additional single-qubit gates for basis transformations
- Total gate count: ~300-400 gates per ansatz layer

This complexity exercises the ZX-IR lowering pipeline for larger molecular Hamiltonians with multiple Pauli terms and validates QASM3 emission for complex variational circuits beyond simple Trotterized models.

## Usage

Compile to QASM3:
```bash
afana lih_12q.cbor.hex --qasm v3
```

The output will be a valid QASM3 program implementing the LiH VQE ansatz with parameterized gates ready for execution on quantum hardware or simulation.