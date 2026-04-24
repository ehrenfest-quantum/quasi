# Floquet-driven Transverse-Field Ising Model (16 qubits)

**Model description**

This example implements a 16‑qubit transverse‑field Ising chain with periodic boundary conditions, driven by a Floquet protocol that alternates between an $X$‑field ($h_x$) and a $Z$‑field ($h_z$) on each Trotter step. The Hamiltonian is

$$
H = \sum_{i=0}^{15} J \; Z_i Z_{i+1} \; + \; h_x \sum_{i=0}^{15} X_i \; + \; h_z \sum_{i=0}^{15} Z_i,
$$

with $Z_{16}\equiv Z_0$ (periodic).  The parameters used are:

- Coupling $J = 1.0$ (GHz·rad)
- Transverse field $h_x = 0.5$ (GHz·rad)
- Longitudinal field $h_z = 0.5$ (GHz·rad)

The system evolves for a total time of **1000 µs** using **10 Trotter steps** (Δt = 100 µs).  The only observable measured is the total energy ⟨H⟩.

**Ehrenfest program fields**

- `version`: 1
- `system.n_qubits`: 16
- `hamiltonian.terms`:
  - $J$ · $Z_i Z_{i+1}$ for each nearest‑neighbour pair (periodic)
  - $h_x$ · $X_i$ on every qubit
  - $h_z$ · $Z_i$ on every qubit
- `evolution.total_us`: 1000.0, `steps`: 10, `dt_us`: 100.0
- `observables`: Energy (`{"type":"E"}`)
- `noise`: minimum $T_1 = 1000\,\mu\text{s}$, $T_2 = 800\,\mu\text{s}$

**Expected behavior**

When compiled with `afana floquet_tfising_16q.cbor.hex --qasm v3` the generated OpenQASM 3 file should contain **> 1000 gates**, reflecting the repeated application of the ZZ coupling and the alternating transverse/longitudinal fields across 10 Trotter steps.  Phase tracking is required for the Floquet driving, so the emitted QASM includes appropriate global phase annotations and multi‑qubit interactions consistent with the Hamiltonian structure.

This example extends the existing 2‑ and 8‑qubit transverse‑field Ising models and serves as a scalability and phase‑consistency test for the ZX‑IR lowering and QASM 3 emission pipelines.