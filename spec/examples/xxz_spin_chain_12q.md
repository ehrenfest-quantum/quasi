# XXZ Spin Chain (12 Qubits)

## Physics Context

The XXZ spin chain is a one-dimensional quantum many-body system with anisotropic Heisenberg interactions. It is a fundamental model in condensed matter physics for studying quantum phase transitions, magnetic ordering, and entanglement dynamics.

### Hamiltonian

$$H = J \sum_{i=0}^{N-2} \left( X_i X_{i+1} + Y_i Y_{i+1} + \Delta Z_i Z_{i+1} \right)$$

Where:
- $J$ is the exchange coupling strength (set to 1.0 GHz·rad)
- $\Delta$ is the anisotropy parameter (set to 0.5)
- $N = 12$ qubits with open boundary conditions
- $X_i, Y_i, Z_i$ are Pauli operators on qubit $i$

### Physical Regimes

- **$\Delta = 0$**: XY model (free fermions)
- **$\Delta = 1$**: Isotropic Heisenberg model (SU(2) symmetric)
- **$\Delta > 1$**: Ising-like antiferromagnet
- **$0 < \Delta < 1$**: XY-like with weak Z coupling (this example)

## Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| `n_qubits` | 12 | Number of spins in the chain |
| `J` | 1.0 GHz·rad | Exchange coupling strength |
| `Δ` (anisotropy) | 0.5 | XXZ anisotropy parameter |
| `total_us` | 100.0 | Total evolution time (μs) |
| `steps` | 10 | Trotter steps |
| `dt_us` | 10.0 | Time step (μs) |
| `t1_us` | 50000 | Minimum T1 requirement (μs) |
| `t2_us` | 20000 | Minimum T2 requirement (μs) |

## Hamiltonian Terms

The Hamiltonian contains 33 two-qubit terms (11 bonds × 3 Pauli combinations):
- 11 XX terms: $X_i X_{i+1}$ with coefficient $J = 1.0$
- 11 YY terms: $Y_i Y_{i+1}$ with coefficient $J = 1.0$
- 11 ZZ terms: $Z_i Z_{i+1}$ with coefficient $J\Delta = 0.5$

## Observable

- **Energy**: Expectation value $\langle H \rangle$ of the full Hamiltonian

## Expected Results

For the ground state of the XXZ chain with $\Delta = 0.5$:
- Energy per bond ≈ -1.2 to -1.4 GHz·rad (depending on initial state preparation)
- The system exhibits gapless spin-liquid behavior for $|\Delta| < 1$
- Correlation functions decay algebraically

## Compilation Target

This example tests:
1. ZX-IR generation for anisotropic Heisenberg models
2. Trotterization of Hamiltonians with different coupling strengths
3. QASM3 emission for 12-qubit circuits
4. Noise analysis against T1/T2 constraints

## Usage

```bash
# Compile to QASM3
cat spec/examples/xxz_spin_chain_12q.cbor.hex | xxd -r -p > /tmp/xxz12.cbor
./target/release/afana /tmp/xxz12.cbor --qasm v3 --stats

# With optimization
./target/release/afana /tmp/xxz12.cbor --qasm v3 --optimize --stats
```

## References

- Bethe, H. (1931). "Zur Theorie der Metalle". *Zeitschrift für Physik*. 71: 205–226.
- Giamarchi, T. (2004). *Quantum Physics in One Dimension*. Oxford University Press.
- Ehrenfest program spec: `spec/ehrenfest-v0.1.cddl`
