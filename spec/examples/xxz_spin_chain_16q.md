# XXZ Spin Chain (16 Qubits) Example

This example implements a 16‑qubit XXZ spin chain with anisotropy parameter `Δ` (Jz) using the Ehrenfest specification.

## Hamiltonian

The Hamiltonian is

```
H = \sum_{i=0}^{14} ( Jx * X_i X_{i+1} + Jy * Y_i Y_{i+1} + Jz * Z_i Z_{i+1} )
```

where:
- `Jx = 1.0`
- `Jy = 1.0`
- `Jz = Δ` (anisotropy, set to `0.5` in this example)

Only nearest‑neighbour interactions are included; periodic boundary conditions are **not** applied.

## Evolution Parameters
- **Total evolution time**: `0.0 µs` (placeholder – the compiler still generates a valid circuit).
- **Trotter steps**: `1`
- **Timestep (`dt_us`)**: `0.0 µs`

## Observables
- Energy expectation value (`E`).

## Noise Constraints
- Minimum `T1` = `1000 µs`
- Minimum `T2` = `500 µs`

The CBOR binary for this program is provided in `xxz_spin_chain_16q.cbor.hex`. It can be deserialized with:

```bash
cat spec/examples/xxz_spin_chain_16q.cbor.hex | xxd -r -p > /tmp/xxz16.cbor
cargo run -p afana -- /tmp/xxz16.cbor --qasm v3
```

The resulting QASM3 output passes structural validation and ZX‑IR graph consistency checks.
