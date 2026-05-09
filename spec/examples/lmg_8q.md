# Lipkin-Meshkov-Glick Model (8 Qubits)

## Physics Overview

The Lipkin-Meshkov-Glick (LMG) model is a collective spin model describing N interacting spins with infinite-range interactions. It serves as a paradigmatic example for studying quantum phase transitions, entanglement scaling, and macroscopic quantum phenomena.

### Hamiltonian

The LMG Hamiltonian is:

```
H = -J/N Σ_{i<j} (σ_x^i σ_x^j + σ_y^i σ_y^j + γ σ_z^i σ_z^j) - h Σ_i σ_z^i
```

Where:
- **J**: Interaction strength (collective coupling)
- **γ**: Anisotropy parameter (γ=1 for isotropic case)
- **h**: Transverse magnetic field
- **N**: Number of qubits (spins)

For this 8-qubit example:
- J = 1.0 GHz·rad
- γ = 0.5 (anisotropic)
- h = 0.5 GHz·rad
- N = 8 qubits

### Physical Significance

The LMG model exhibits:
1. **Quantum Phase Transition**: At critical field h_c = J, the system transitions from a symmetry-broken phase to a symmetric phase
2. **Entanglement Scaling**: Near criticality, entanglement entropy scales logarithmically with system size
3. **Collective Behavior**: All-to-all coupling makes it ideal for studying macroscopic quantum coherence

## Program Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 8 | Number of spins |
| J | 1.0 | Collective coupling strength |
| γ | 0.5 | Anisotropy parameter |
| h | 0.5 | Transverse field |
| total_us | 100.0 | Total evolution time (μs) |
| steps | 10 | Trotter steps |
| dt_us | 10.0 | Time step (μs) |
| t1_us | 1000.0 | Minimum T1 requirement (μs) |
| t2_us | 500.0 | Minimum T2 requirement (μs) |

## Observables

1. **Energy (E)**: Expectation value ⟨H⟩ of the full Hamiltonian
2. **Sigma-Z on qubit 0**: Local magnetization ⟨σ_z^0⟩

## Expected Results

For the given parameters:
- Ground state energy ≈ -4.5 GHz·rad (variational estimate)
- Local magnetization ⟨σ_z^0⟩ ≈ -0.3 to -0.5 (depending on field strength)
- Entanglement entropy should show collective behavior characteristic of all-to-all coupled systems

## Compilation

```bash
# Compile to QASM3
cat spec/examples/lmg_8q.cbor.hex | xxd -r -p > /tmp/lmg_8q.cbor
./target/release/afana /tmp/lmg_8q.cbor --qasm v3 --optimize --stats
```

## References

1. Lipkin, H. J., Meshkov, N., & Glick, A. J. (1965). Validity of many-body approximation methods for a solvable model. *Nuclear Physics*, 62(2), 188-212.
2. Vidal, G., Dusuel, S., & Barthel, T. (2007). Entanglement entropy in the Lipkin-Meshkov-Glick model. *Journal of Statistical Mechanics*, P01015.
3. Ribeiro, P., Vidal, J., & Mosseri, R. (2008). Exact spectrum of the Lipkin-Meshkov-Glick model in the thermodynamic limit. *Physical Review E*, 78(2), 021106.

---
*Generated for QUASI Ehrenfest benchmark suite — Phase PHASE-007*