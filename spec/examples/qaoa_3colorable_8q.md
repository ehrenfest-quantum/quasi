# QAOA for 3-Colorable Graph (8 Qubits)

## Physics Overview

This Ehrenfest program encodes the Quantum Approximate Optimization Algorithm (QAOA) for solving the 3-coloring problem on a graph. The 3-coloring problem asks: can we assign one of three colors to each vertex such that no two adjacent vertices share the same color?

## Encoding

**8 qubits** encoding a 4-vertex graph with 3-coloring:
- Qubits 0-1: Vertex 0 color (00, 01, 10 = 3 colors)
- Qubits 2-3: Vertex 1 color
- Qubits 4-5: Vertex 2 color  
- Qubits 6-7: Vertex 3 color

## Hamiltonian

The QAOA Hamiltonian consists of:

### Problem Hamiltonian (H_P)
Penalizes adjacent vertices having the same color using ZZ interactions:
- Terms 0-3: Edge constraints (ZZ interactions between vertex qubit pairs)
- Coefficient: +1.0 GHz·rad per edge violation

### Mixer Hamiltonian (H_M)
Transverse field enabling quantum tunneling between color states:
- Terms 4-11: Single-qubit X operators on all 8 qubits
- Coefficient: -1.0 GHz·rad (standard QAOA mixer)

## Parameters

| Parameter | Value |
|-----------|-------|
| Qubits | 8 |
| Total evolution time | 100 μs |
| Trotter steps | 5 |
| Time step (dt) | 20 μs |
| T1 requirement | 100,000 μs |
| T2 requirement | 100,000 μs |

## Observables

- **Energy (E)**: Expectation value of the full Hamiltonian ⟨H⟩

## Expected Results

For a valid 3-coloring:
- Ground state energy should be near zero (no edge violations)
- The QAOA circuit should find low-energy states corresponding to valid colorings

## Graph Structure

This example encodes a 4-vertex graph with edges:
- Vertex 0 ↔ Vertex 1 (qubits 0,1 ↔ 2,3)
- Vertex 1 ↔ Vertex 2 (qubits 2,3 ↔ 4,5)
- Vertex 2 ↔ Vertex 3 (qubits 4,5 ↔ 6,7)
- Vertex 3 ↔ Vertex 0 (qubits 6,7 ↔ 0,1)

This forms a cycle graph C4, which is 2-colorable (and thus 3-colorable).

## Usage

```bash
# Compile to QASM3
cat spec/examples/qaoa_3colorable_8q.cbor.hex | xxd -r -p > /tmp/qaoa.cbor
./target/release/afana /tmp/qaoa.cbor --qasm v3

# With optimization
./target/release/afana /tmp/qaoa.cbor --qasm v3 --optimize --stats
```

## Validation

```bash
# Verify CBOR deserialization
cat spec/examples/qaoa_3colorable_8q.cbor.hex | xxd -r -p | afana --validate

# Compile and validate QASM3
afana spec/examples/qaoa_3colorable_8q.cbor.hex --qasm v3 | qasm3-validate
```
