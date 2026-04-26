# Solvayeur Scheduling Hamiltonian (8 backends, 3 qubits each)

This example models the scheduling Hamiltonian used by the Solvayeur kernel (ATW algorithm) to dispatch workloads across 8 quantum backends, each with 3 qubits. The total system comprises 24 qubits.

## Physics

The scheduling Hamiltonian is:

$$H(k) = \sum_{ij} J_{ij} Z_i Z_j + \sum_i h_i(k) Z_i + \Gamma \sum_i X_i$$

- **ZZ couplings** ($J_{ij}$): Represent contention between qubits within the same backend and across backends. Intra-backend couplings (e.g., qubits 0-1, 3-4) model shared resources; cross-backend couplings (e.g., qubits 2-3) model distributed entanglement requirements.
- **Local Z fields** ($h_i(k)$): Learned bias terms that encode historical performance of each backend. These are updated after each scheduling round.
- **Transverse field** ($\Gamma$): Exploration term that drives the system away from local optima, annealing over time.

## Parameters

| Parameter | Value |
|-----------|-------|
| `n_qubits` | 24 (8 backends × 3 qubits) |
| `total_us` | 10.0 |
| `steps` | 100 |
| `dt_us` | 0.1 |
| `constant_offset` | 0.0 |
| `t1_us` | 100.0 |
| `t2_us` | 50.0 |

## Hamiltonian Terms

- 3 ZZ coupling terms: (0,1), (3,4), (2,3) — the last is cross-backend
- 2 local Z field terms: qubits 0 and 1
- 2 transverse X field terms: qubits 0 and 1

## Expected Results

When compiled with `afana <file> --qasm v3`, the output should be a valid QASM3 circuit using at least 24 qubits. The circuit should contain non-trivial coupling terms (CX gates) derived from the ZZ interactions, and the total gate count should reflect the Trotterization of the Hamiltonian.

## Usage

```bash
xxd -r -p spec/examples/solvayeur_scheduling_8b3q.cbor.hex > /tmp/prog.cbor
cargo run -p afana --release -- /tmp/prog.cbor --qasm v3 --stats
```
