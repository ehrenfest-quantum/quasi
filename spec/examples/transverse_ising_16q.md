# Transverse-field Ising Model (16 qubits)

## Physics

The Transverse-field Ising model (TFIM) is a fundamental quantum many-body system:

```
H = -J Σᵢ ZᵢZᵢ₊₁ - h Σᵢ Xᵢ
```

Where:
- **J**: Nearest-neighbor ZZ coupling strength (default: 0.5 GHz·rad)
- **h**: Transverse field strength (default: 0.3 GHz·rad)
- **n_qubits**: 16 qubits with open boundary conditions

This model exhibits a quantum phase transition at h/J = 1 in the thermodynamic limit.

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 16 | Number of qubits |
| J | 0.5 | ZZ coupling (GHz·rad) |
| h | 0.3 | Transverse field (GHz·rad) |
| total_us | 10.0 | Total evolution time (μs) |
| steps | 5 | Trotter steps |
| dt_us | 2.0 | Time step (μs) |
| t1_us | 100.0 | Minimum T1 requirement (μs) |
| t2_us | 50.0 | Minimum T2 requirement (μs) |

## Hamiltonian Terms

- **15 ZZ terms**: Z₀Z₁, Z₁Z₂, ..., Z₁₄Z₁₅ (open boundary)
- **16 X terms**: X₀, X₁, ..., X₁₅ (transverse field)
- **Total**: 31 Pauli terms

## Expected Results

### Compilation

- **Trotter order**: First-order (configurable to 2nd or 4th)
- **Gate count**: ~465 gates per Trotter step (15 ZZ × 3 gates + 16 X × 3 gates)
- **Total gates**: ~2325 gates for 5 steps
- **ZX-IR spiders**: >200 (depends on gate decomposition)

### Validation

- ZX-IR graph validates without errors
- All 16 qubits represented in input/output boundaries
- Hamiltonian terms validated for qubit range
- Trotter step consistency: dt_us = total_us / steps

### Observables

- Energy expectation ⟨H⟩
- Optional: σᶻ on individual qubits for magnetization

## Usage

```bash
# Compile to QASM3 with optimization
cat spec/examples/transverse_ising_16q.cbor.hex | xxd -r -p > /tmp/tf16.cbor
./target/release/afana /tmp/tf16.cbor --qasm v3 --optimize --stats --trotter-order 2
```

## Commutation Relationships

The TFIM Hamiltonian has specific commutation properties:

1. **ZZ-ZZ**: All ZZ terms commute (share Z operators on overlapping qubits)
2. **X-X**: All X terms commute (act on different qubits)
3. **ZZ-X**: Terms on disjoint qubits commute; terms sharing a qubit do NOT commute

This non-commutativity is why Trotterization introduces approximation error, which decreases with more steps (O(dt) for first-order, O(dt²) for second-order).

## References

- Pfeuty, P. (1970). The one-dimensional Ising model with a transverse field. Annals of Physics.
- Sachdev, S. (2011). Quantum Phase Transitions. Cambridge University Press.
