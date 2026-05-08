# Floquet Hamiltonian with Time-Dependent Driving Field (12 qubits)

## Physics Model

This Ehrenfest program describes a **Floquet system** — a quantum system with time-periodic Hamiltonian. The model consists of:

1. **Static ZZ interactions**: Nearest-neighbor Ising-type coupling along a 12-qubit chain
2. **Time-periodic X driving**: Global transverse field with periodic amplitude

### Hamiltonian

```
H(t) = J Σᵢ=₀¹¹ ZᵢZᵢ₊₁ + Ω(t) Σᵢ=₀¹¹ Xᵢ
```

Where:
- `J = 0.5 GHz·rad` — ZZ coupling strength
- `Ω(t)` — periodic driving field (captured via Trotter discretization)
- 12 qubits in a linear chain

## Ehrenfest Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| `n_qubits` | 12 | Linear chain |
| `total_us` | 10.0 | Total evolution time (μs) |
| `steps` | 20 | Trotter steps |
| `dt_us` | 0.5 | Time step (μs) |
| `t1_us` | 100.0 | Minimum T1 relaxation time |
| `t2_us` | 50.0 | Minimum T2 coherence time |

## Hamiltonian Terms

- **11 ZZ terms**: Z₀Z₁, Z₁Z₂, ..., Z₁₀Z₁₁ (coupling strength 0.5)
- **12 X terms**: X₀, X₁, ..., X₁₁ (driving strength 0.3)

Total: 23 Pauli terms

## Observables

1. **SZ on qubit 0**: ⟨Z₀⟩ — local magnetization
2. **Energy**: ⟨H⟩ — expectation value of full Hamiltonian

## Expected Results

### Trotterization (1st order, 20 steps)
- Total gates: ~2000+ (depends on Trotter order)
- Gate types: H, Rz, CX (for ZZ ladders and X basis changes)
- Circuit depth: proportional to steps × terms

### ZX-IR Graph
- Spider count: >100 (inputs, outputs, and gate spiders)
- Edge count: >100 (connections between spiders)
- Valid ZX graph with proper boundary nodes

### QASM3 Output
- Valid OpenQASM 3.0 with `stdgates.inc`
- Gate sequence: H for X-basis, Rz for rotations, CX for ZZ entanglement
- Measurements on all 12 qubits

## Usage

```bash
# Compile to QASM3
cat spec/ehrenfest/examples/floquet_time_dependent_12q.cbor.hex | xxd -r -p > /tmp/floquet.cbor
./target/release/afana /tmp/floquet.cbor --qasm v3 --trotter-order 2 --stats

# With ZX optimization
./target/release/afana /tmp/floquet.cbor --qasm v3 --optimize --stats
```

## Floquet Physics Notes

Floquet systems exhibit **time-periodic dynamics** where the Hamiltonian repeats with period T. In Ehrenfest, the periodicity is captured through:

1. **Trotter discretization**: The time evolution e^{-iHt} is approximated as product of short-time evolutions
2. **Repeated application**: Each Trotter step applies all Hamiltonian terms
3. **Effective Floquet Hamiltonian**: For high-frequency driving, the system behaves according to an effective static Hamiltonian

This example demonstrates Afana's ability to handle time-dependent physics through standard Trotter-Suzuki decomposition.

## References

- Floquet theory: https://en.wikipedia.org/wiki/Floquet_theory
- Trotter-Suzuki decomposition: trotter.rs in afana/
- Ehrenfest spec: spec/ehrenfest-v0.1.cddl
