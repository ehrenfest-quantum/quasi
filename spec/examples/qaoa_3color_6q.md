# QAOA for 3-Colorable Graph (6 Qubits)

## Physics Overview

This Ehrenfest program implements a Quantum Approximate Optimization Algorithm (QAOA) for finding valid 3-colorings of a cycle graph C₆ (6 vertices arranged in a ring). The problem is encoded as a cost Hamiltonian that penalizes adjacent vertices having the same color state, combined with a transverse-field mixer Hamiltonian.

## Graph Topology

**Graph**: Cycle graph C₆ (6 vertices, 6 edges)

```
    0 —— 1
   /      \
  5        2
   \      /
    4 —— 3
```

**Edges**: (0,1), (1,2), (2,3), (3,4), (4,5), (5,0)

**3-colorability**: A cycle with even length is 2-colorable (bipartite), hence also 3-colorable. Valid colorings exist where no two adjacent vertices share the same color.

## Hamiltonian Encoding

### Cost Hamiltonian H_C

The cost Hamiltonian penalizes adjacent vertices in the same state using ZZ interactions:

```
H_C = Σ(i,j)∈edges Zi Zj
```

**Terms** (6 ZZ interaction terms):
| Term | Qubits | Coefficient | Pauli String |
|------|--------|-------------|--------------|
| 1 | 0, 1 | +1.0 | Z₀Z₁ |
| 2 | 1, 2 | +1.0 | Z₁Z₂ |
| 3 | 2, 3 | +1.0 | Z₂Z₃ |
| 4 | 3, 4 | +1.0 | Z₃Z₄ |
| 5 | 4, 5 | +1.0 | Z₄Z₅ |
| 6 | 5, 0 | +1.0 | Z₅Z₀ |

### Mixer Hamiltonian H M

The mixer uses transverse X fields to enable transitions between states:

```
HM = Σi Xi
```

**Terms** (6 single-qubit X terms):
| Term | Qubit | Coefficient | Pauli |
|------|-------|-------------|-------|
| 1 | 0 | +1.0 | X₀ |
| 2 | 1 | +1.0 | X₁ |
| 3 | 2 | +1.0 | X₂ |
| 4 | 3 | +1.0 | X₃ |
| 5 | 4 | +1.0 | X₄ |
| 6 | 5 | +1.0 | X₅ |

### Total Hamiltonian

```
H = HC + HM = Σ(i,j)∈edges ZiZj + Σi Xi
```

## Program Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| n_qubits | 6 | One qubit per graph vertex |
| total us | 10.0 | Total evolution time (μs) |
| steps | 10 | Trotterization steps |
| dt us | 1.0 | Time step = total/steps |
| t1 us | 100.0 | Minimum T1 requirement |
| t2 us | 50.0 | Minimum T2 requirement |

## Observables

- **Energy (E)**: Expectation value ⟨H⟩ of the full Hamiltonian

## Expected Results

After Trotterized evolution:
- The ground state corresponds to valid 3-colorings of C₆
- Energy expectation should approach the minimum eigenvalue
- For C₆, valid colorings have zero penalty (all adjacent vertices different)
- The mixer ensures exploration of the solution space

## Compilation Pipeline

1. **CBOR deserialization**: `afana::cbor::from_cbor()` parses the binary
2. **Type checking**: Validates qubit indices, noise constraints
3. **Trotterization**: Converts Hamiltonian to gate sequence
4. **ZX optimization**: Reduces gate count via ZX-calculus rewriting
5. **QASM3 emission**: Outputs executable quantum circuit

## Usage

```bash
# Compile to QASM3
cat spec/examples/qaoa_3color_6q.cbor.hex | xxd -r -p > /tmp/qaoa.cbor
./target/release/afana /tmp/qaoa.cbor --qasm v3 --optimize --stats
```

## Testing

```rust
use afana::cbor::from_cbor;

let hex = include_str!("../spec/examples/qaoa_3color_6q.cbor.hex");
let bytes = hex::decode(hex.trim()).unwrap();
let program = from_cbor(&bytes).unwrap();
assert_eq!(program.system.n_qubits, 6);
assert_eq!(program.hamiltonian.terms.len(), 12); // 6 ZZ + 6 X terms
```

## References

- Farhi, E., Goldstone, J., & Gutmann, S. (2014). A Quantum Approximate Optimization Algorithm. arXiv:1411.4028
- Graph coloring via QAOA is a standard benchmark for combinatorial optimization on quantum devices
