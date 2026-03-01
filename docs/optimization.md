# Afana Optimization Passes

Afana provides two complementary optimization passes for reducing gate counts in compiled quantum circuits.

## T-Gate Cancellation Pass

**Function:** `afana.reduce_t_gates(qasm: str) -> (str, dict)`

T gates (π/8 rotations) are the most resource-intensive gates in fault-tolerant quantum computing — they require magic state distillation in standard error-correction codes and dominate both compile time and execution cost. The T-gate cancellation pass exploits the algebraic identity T⁸ = I (identity) to fold runs of consecutive T / Tdg gates on the same qubit into their minimal representation.

### Reduction table

| Accumulated T count (mod 8) | Emitted gate(s) |
|-----------------------------|-----------------|
| 0 | *(none — identity)* |
| 1 | `t` |
| 2 | `s` |
| 3 | `s`, `t` |
| 4 | `z` |
| 5 | `z`, `t` |
| 6 | `sdg` |
| 7 | `tdg` |

### Key properties

- **T / Tdg on different qubits commute** (both diagonal in the Z basis), so the pass accumulates T-phases per qubit and can merge `t q[0]; t q[1]; t q[0];` into `s q[0]; t q[1];`.
- T-phase runs are flushed at the first non-T gate on the same qubit (e.g. H, CNOT), preserving circuit semantics.
- The pass requires **no external dependencies** — it operates on the QASM text directly.

### Example

Input circuit:
```
OPENQASM 2.0;
include "qelib1.inc";
qreg q[1];
t q[0];
t q[0];
t q[0];
t q[0];
```

After `reduce_t_gates`:
```
OPENQASM 2.0;
include "qelib1.inc";
qreg q[1];
z q[0];
```

Four T gates collapse to a single Z gate (saving 3 gates).

### CLI usage

```bash
# Compile with T-gate reduction only
afana compile circuit.qasm --reduce-t

# Combine T-gate reduction with full ZX-calculus optimization
afana compile circuit.qasm --reduce-t --optimize

# Benchmark gate counts
afana benchmark *.qasm --reduce-t
```

The CLI reports T-gate savings before and after the pass:
```
circuit.qasm: t_before=4 t_after=0 (T-gate reduction saved 4 gate(s))
circuit.qasm: before=4 after=1
```

### Python API

```python
from afana import reduce_t_gates, optimize_qasm_with_stats

# Standalone T-gate pass
qasm_reduced, stats = reduce_t_gates(qasm)
print(f"T gates: {stats['t_before']} → {stats['t_after']}")

# Integrated with ZX optimization
qasm_opt, stats = optimize_qasm_with_stats(qasm, reduce_t=True)
```

---

## ZX-Calculus Optimization Pass

**Function:** `afana.optimize_qasm(qasm: str, reduce_t: bool = False) -> str`

When [PyZX](https://pyzx.readthedocs.io/) is available, Afana applies a `full_reduce` simplification using the ZX-calculus graph rewrite rules:

- **Spider fusion** — merge adjacent spiders of the same colour
- **Identity removal** — eliminate identity spiders
- **Colour change** — convert between Z and X spiders via Hadamard edges
- **Pivot rule** — eliminate interior connected pairs
- **Local complementation** — simplify neighbourhood structures

The ZX pass operates on the ZX-IR (a graph of Z-spiders, X-spiders, and Hadamard edges) and extracts a minimal gate sequence after simplification. Typical reduction on random circuits: **30–50% fewer gates**.

The pass has a **never-worse guarantee**: if the optimized circuit has more gates than the input, the original is returned unchanged.

### Combining passes

For fault-tolerant targets, combining both passes is recommended:

```bash
afana compile bell.qasm --reduce-t --optimize
```

The T-gate cancellation runs first (reducing the most expensive gates), then ZX simplification reduces the remaining Clifford overhead.
