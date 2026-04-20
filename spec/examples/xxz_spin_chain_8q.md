# XXZ Spin Chain (8 qubits)

## Physics

The XXZ spin chain is a fundamental model in quantum magnetism that extends the Heisenberg model with anisotropic coupling in the Z direction:

$$H = J \sum_{i=0}^{N-2} (X_i X_{i+1} + Y_i Y_{i+1} + \Delta Z_i Z_{i+1})$$

Where:
- $J$ is the exchange coupling strength (set to 1.0 GHz·rad)
- $\Delta$ is the anisotropy parameter (set to 0.5)
- $N = 8$ qubits with open boundary conditions

This model interpolates between:
- $\Delta = 0$: XY model (pure transverse coupling)
- $\Delta = 1$: Heisenberg XXX model (isotropic)
- $\Delta \to \infty$: Ising model (pure longitudinal coupling)

## Hamiltonian Terms

The 8-qubit XXZ chain has 21 Pauli terms (7 nearest-neighbor bonds × 3 interaction types):

**XX interactions (7 terms):**
- $J \cdot X_0 X_1$, $J \cdot X_1 X_2$, $J \cdot X_2 X_3$, $J \cdot X_3 X_4$, $J \cdot X_4 X_5$, $J \cdot X_5 X_6$, $J \cdot X_6 X_7$

**YY interactions (7 terms):**
- $J \cdot Y_0 Y_1$, $J \cdot Y_1 Y_2$, $J \cdot Y_2 Y_3$, $J \cdot Y_3 Y_4$, $J \cdot Y_4 Y_5$, $J \cdot Y_5 Y_6$, $J \cdot Y_6 Y_7$

**ZZ interactions (7 terms):**
- $J \cdot \Delta \cdot Z_0 Z_1$, $J \cdot \Delta \cdot Z_1 Z_2$, ..., $J \cdot \Delta \cdot Z_6 Z_7$

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Qubits | 8 | Linear chain with open boundaries |
| J (exchange) | 1.0 GHz·rad | Coupling strength |
| Δ (anisotropy) | 0.5 | XXZ anisotropy parameter |
| Total time | 100.0 μs | Evolution duration |
| Trotter steps | 10 | First-order Trotterization |
| dt | 10.0 μs | Time step |
| T1 requirement | 1000.0 μs | Minimum relaxation time |
| T2 requirement | 500.0 μs | Minimum dephasing time |

## Observables

- **Energy**: Expectation value ⟨H⟩ of the full Hamiltonian
- **σᶻ on qubit 0**: Local magnetization at chain edge
- **σᶻ on qubit 3**: Local magnetization in chain bulk

## Expected Results

After Trotter evolution:
- Energy should be conserved within Trotter error (~1% for 10 steps)
- Magnetization oscillates due to XX/YY terms
- ZZ anisotropy suppresses transverse spin fluctuations

## Compilation

```bash
# Compile to QASM3
cat spec/examples/xxz_spin_chain_8q.cbor.hex | xxd -r -p > /tmp/xxz8.cbor
./target/release/afana /tmp/xxz8.cbor --qasm v3 --optimize --stats
```

Expected gate types in output: `rx`, `rz`, `cz` (from ZX-IR lowering of XX, YY, ZZ interactions).

## CBOR Schema

This example uses Ehrenfest v0.1 schema (`spec/ehrenfest-v0.1.cddl`).
