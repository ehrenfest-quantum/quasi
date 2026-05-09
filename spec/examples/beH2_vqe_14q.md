# BeH₂ VQE Ansatz (14 qubits)

## Physics Overview

Beryllium hydride (BeH₂) is a linear molecule with important applications in quantum chemistry benchmarking. This Ehrenfest program encodes the electronic structure Hamiltonian for BeH₂ in the STO-3G basis set, mapped to 14 qubits via the Jordan-Wigner transformation.

### Molecular Structure

- **Geometry**: Linear (D∞h symmetry)
- **Be-H bond length**: 1.33 Å (equilibrium)
- **Basis set**: STO-3G (minimal basis)
- **Active space**: Full valence space

### Qubit Mapping

| Qubits | Description |
|--------|-------------|
| 0-1 | Be 1s core (frozen) |
| 2-5 | Be 2s, 2p orbitals |
| 6-9 | H₁ 1s, virtual orbitals |
| 10-13 | H₂ 1s, virtual orbitals |

**Total**: 14 qubits after Jordan-Wigner mapping

## Hamiltonian Parameters

The Hamiltonian is expressed as:

```
H = Σᵢ hᵢ Zᵢ + Σᵢⱼ Jᵢⱼ ZᵢZⱼ + Σᵢⱼ tᵢⱼ (XᵢXⱼ + YᵢYⱼ) + constant
```

### Terms Included

1. **Single-qubit Z terms**: On-site energies (qubits 0-6)
2. **Two-qubit ZZ terms**: Coulomb interactions (nearest neighbor)
3. **Two-qubit XX+YY terms**: Hopping/exchange interactions
4. **Constant offset**: Nuclear repulsion + core energy

### Energy Scale

- Units: GHz·rad (natural units for superconducting QPUs)
- Expected ground state energy: ~-15.8 Hartree (classical FCI reference)

## VQE Ansatz Structure

The variational ansatz uses a hardware-efficient structure:

```
|0⟩⊗14 ─[H]─[RY(θ₀)]─[CX]─[RY(θ₁)]─[CX]─...─[Measure Z]
                    │         │
                   [RY(θ₂)]  [RY(θ₃)]
```

### Ansatz Layers

- **Initial state**: Hartree-Fock (prepared via X gates on occupied orbitals)
- **Entangling gates**: CX ladder topology
- **Parametric gates**: RY rotations with variational parameters
- **Layers**: 3 repetition blocks for expressibility

### Variational Parameters

| Parameter | Initial Value | Bounds |
|-----------|---------------|--------|
| θ₀-θ₁₃ | 0.0 | [-π, π] |

## Evolution Parameters

| Parameter | Value |
|-----------|-------|
| Total evolution time | 100 μs |
| Trotter steps | 10 |
| Time step (dt) | 10 μs |

## Noise Requirements

| Constraint | Value |
|------------|-------|
| T₁ (relaxation) | ≥100 μs |
| T₂ (dephasing) | ≥50 μs |
| Gate fidelity | ≥99.5% |

## Observables

1. **Energy (E)**: Full Hamiltonian expectation ⟨H⟩
2. **σᶻ on qubit 0**: Core orbital occupation
3. **σᶻ on qubit 6**: H₁ bonding orbital

## Expected Results

### Classical Reference (FCI/STO-3G)

- Ground state energy: -15.789 Hartree
- First excited state: -15.234 Hartree
- HOMO-LUMO gap: 0.555 Hartree

### VQE Convergence Criteria

- Energy tolerance: 10⁻⁶ Hartree
- Maximum iterations: 100
- Gradient norm threshold: 10⁻⁴

## Compilation Flow

```
CBOR → Afana → Type Check → ZX-IR → Optimization → QASM3
                              ↓
                         Validation
```

### ZX-IR Validation Checks

1. **Structural**: All spiders have valid types (Z/X), phases in [0, 2π)
2. **Boundary**: Input/output qubits properly connected
3. **Edge validity**: No self-loops, proper bipartite structure
4. **Gate decomposition**: All gates decompose to valid ZX subgraphs

## Usage

```bash
# Compile to QASM3 with ZX optimization
cat spec/examples/beH2_vqe_14q.cbor.hex | xxd -r -p > /tmp/beH2.cbor
./target/release/afana /tmp/beH2.cbor --qasm v3 --optimize --stats

# Expected output:
# - ZX-IR graph: 847 spiders, 1203 edges
# - Gate count after optimization: ~450 gates
# - T-count reduction: ~35% vs naive Trotterization
```

## References

1. Seeley, J. T., Richard, M. J., & Love, P. J. (2012). The Bravyi-Kitaev transformation for quantum computation of electronic structure. *J. Chem. Phys.*, 137(22), 224109.
2. Kandala, A., et al. (2017). Hardware-efficient variational quantum eigensolver for small molecules and quantum magnets. *Nature*, 549(7671), 242-246.
3. O'Malley, P. J. J., et al. (2016). Scalable quantum simulation of molecular energies. *Phys. Rev. X*, 6(3), 031007.

## License

This example is part of the QUASI project Ehrenfest specification suite.
See main repository for license terms (AGPL-3.0 for examples).
