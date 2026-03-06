# Ehrenfest Examples

Ehrenfest programs are CBOR binary. There is no text form.

Examples are in `spec/examples/` as CBOR hex dumps with companion documentation:

| Example | Qubits | Physics |
|---------|--------|---------|
| [`rabi_oscillation_1q`](../spec/examples/rabi_oscillation_1q.md) | 1 | Single-qubit Rabi oscillation (σ_x drive) |
| [`transverse_ising_2q`](../spec/examples/transverse_ising_2q.md) | 2 | Transverse-field Ising model (σ_x·σ_x + σ_z) |
| [`heisenberg_4q`](../spec/examples/heisenberg_4q.md) | 4 | Heisenberg spin chain (XXX model) |
| [`vqe_h2_parametric`](../spec/examples/vqe_h2_parametric.cbor.hex) | 2 | Variational H₂ ground state (v0.2 parametric) |

To compile an example:

```bash
# Decode hex to binary CBOR, then compile
xxd -r -p spec/examples/rabi_oscillation_1q.cbor.hex > /tmp/rabi.cbor
afana /tmp/rabi.cbor --qasm v3 --stats
```
