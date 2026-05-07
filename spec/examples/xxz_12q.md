# XXZ Spin Chain (12 qubits, Δ=1.5)

## Physics

The XXZ spin chain is a fundamental model in condensed matter physics describing interacting spins with anisotropic coupling:

```
H = J * Σᵢ (XᵢXᵢ₊₁ + YᵢYᵢ₊₁ + Δ * ZᵢZᵢ₊₁)
```

Where:
- **J = 0.5** — Exchange coupling strength (GHz·rad)
- **Δ = 1.5** — Anisotropy parameter (Δ > 1: Ising-like, Δ < 1: XY-like)
- **12 qubits** with nearest-neighbor coupling (11 interaction terms per type)

This model exhibits a quantum phase transition at Δ = 1 and is a key benchmark for Trotter step consistency validation.

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 12 | Chain length |
| J | 0.5 | Exchange coupling |
| Δ (delta) | 1.5 | Anisotropy (Ising-like regime) |
| total_us | 1.0 | Total evolution time (μs) |
| steps | 10 | Trotter steps |
| dt_us | 0.1 | Time step (μs) |
| t1_us | 100.0 | T1 relaxation requirement |
| t2_us | 50.0 | T2 coherence requirement |

## Hamiltonian Terms

33 total Pauli terms (11 pairs × 3 types):
- 11 × XX terms (coefficient: 0.5)
- 11 × YY terms (coefficient: 0.5)
- 11 × ZZ terms (coefficient: 0.75 = 0.5 × 1.5)

## Expected Results

### Trotter Step Consistency
- **Coefficient preservation**: All 33 term coefficients preserved within 1e-10
- **Gate sequence uniformity**: Each Trotter step produces identical gate pattern
- **Term commutation**: XX, YY, ZZ structure maintained across steps

### Compilation
- **ZX-IR spiders**: ~400-600 (depends on Trotter order)
- **QASM3 gates**: ~200-400 (first-order), ~1000-2000 (second-order)
- **Estimated fidelity**: > 0.95 (within T2 budget)

## Usage

```bash
# Compile to QASM3
cat spec/examples/xxz_12q.cbor.hex | xxd -r -p > /tmp/xxz.cbor
./target/release/afana /tmp/xxz.cbor --qasm v3 --stats

# Run validation test
cargo test -p afana test_xxzz_trotter_consistency_12q
```

## References

- Lieb, E. H., Schultz, T., & Mattis, D. (1961). Two soluble models of an antiferromagnetic chain. *Annals of Physics*, 16(3), 407-466.
- Giamarchi, T. (2003). *Quantum Physics in One Dimension*. Oxford University Press.