# Afana Optimization

Afana's `--optimize` flag runs the ZX optimization pipeline and now includes a
lightweight T-gate reduction pass before the PyZX simplifier runs.

## T-gate reduction

The optimizer looks for adjacent `T` gates on the same qubit and rewrites them
to the equivalent `S` gate. This reduces the T-count without increasing total
gate count.

Example input:

```qasm
OPENQASM 2.0;
include "qelib1.inc";
qreg q[1];
t q[0];
t q[0];
h q[0];
```

Run the optimization via the CLI:

```bash
python3 -m afana.cli compile sample.qasm --optimize
```

Expected report:

```text
sample.qasm: before=3 after=2 t_before=2 t_after=0
```

Use `benchmark` to compare several circuits at once:

```bash
python3 -m afana.cli benchmark sample.qasm other.qasm --optimize
```
