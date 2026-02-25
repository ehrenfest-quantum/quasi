# Ehrenfest AI Generation Guide

## 1. Mental Model

You are not writing quantum circuits. Describe the physical system and measurement objectives. The Afana compiler handles gate decomposition.

```
System = Hamiltonian + NoiseConstraints + Observables
```

## 2. Hamiltonian Catalog

| Problem               | Hamiltonian Formula                   | Pauli Terms       |
|-----------------------|---------------------------------------|-------------------|
| Transverse-field Ising| H = -JΣZ_iZ_j - hΣX_i                 | ZZ, X             |
| Heisenberg XXX        | H = JΣ(X_iX_j + Y_iY_j + Z_iZ_j)      | XX, YY, ZZ        |
| QAOA MaxCut           | H_C = ½Σ(1 - Z_iZ_j) per edge         | ZZ                |
| VQE H2 Molecule       | H = 0.5Z₁ + 0.5Z₂ - 0.3X₁X₂ + ...     | ZZ, XX, YY, Z, X  |
| Rabi Oscillation      | H = -½ΩX                              | X                 |

## 3. Noise Requirements

- T2 ≥ 10 × total_evolution_time
- T1 ≥ 5 × T2 (T2 cannot exceed 2×T1)
- Gate fidelity requirements:
  - 99% fidelity → ≥0.99
  - 99.9% fidelity → ≥0.999
  - 99.99% fidelity → ≥0.9999

## 4. Observable Selection

- **SZ**: Z-basis measurement (computational basis)
- **SX**: X-basis measurement
- **E**: Expectation value of Hamiltonian
- **rho**: Full density matrix
- **F**: Fidelity with target state

## 5. Common Mistakes

1. **T2 > 2×T1**: Physically impossible due to T2 ≤ 2T1 relation
2. **Evolution time > T2**: Results in complete decoherence
3. **Ignoring backend connectivity**: Generate topology-aware terms
4. **Noise-less simulations**: Always include T1/T2 for real devices

## 6. Worked Examples

### Transverse Ising Model
```
hamiltonian {
  terms [
    {pauli: "ZZ", qubits: [0,1], coefficient: -1.0},
    {pauli: "X", qubits: [0], coefficient: -0.5}
  ]
}
observables { sz {} }
noise { t1: 50000, t2: 25000 }
```

### Rabi Oscillation
```
hamiltonian {
  terms [{pauli: "X", qubits: [0], coefficient: -0.3}]
}
time_evolution: 15.0  # 15 ns
observables { sx {} }
```

---

*Contribution Metadata*
```
Contribution-Agent: claude-3.5-sonnet
Task: QUASI-016
Verification: ci-pass
```