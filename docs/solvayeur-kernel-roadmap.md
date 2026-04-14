# Solvayeur Kernel Roadmap

**The path from load balancer to quantum OS kernel.**

Each phase adds capabilities to the Solvayeur. The mechanism stays the same — an Ehrenfest program compiled by Afana, measured for decisions. The Hamiltonian gets richer. The kernel manages more.

---

## Phase A — Backend Selection (DONE)

**What the kernel decides:** Which backend runs this workload.

**Hamiltonian:** n = ⌈log₂ backends⌉ qubits. ZZ contention + Z bias + X exploration.

**What exists:**
- ATW loop: compile → measure → dispatch → observe → update bias
- 14 backend profiles with HAL Contract capabilities
- Huoma profiler bridge (classical/quantum routing)
- Real QPU verification: 10 ATW rounds on IBM Strasbourg
- Mock and live QPU execution modes

**Limitation:** The kernel sees workloads as atomic units. It doesn't look inside the circuit. It doesn't manage individual qubits. It's a job dispatcher, not an OS kernel.

---

## Phase B — Qubit Region Allocation

**What the kernel decides:** Which physical qubits on the selected backend are assigned to this program.

**Why it matters:** QPU qubits are not equal. Qubit 47 might have T1 = 300μs while qubit 12 has T1 = 80μs. Two-qubit gate fidelity varies by edge. The transpiler currently picks qubits blindly (or greedily). The kernel should pick qubits informed by calibration data, avoiding degraded regions and reserving good regions for high-priority work.

**Hamiltonian extension:**
```
H_B = H_A + Σ_q c_q Z_q    (qubit quality bias)
           + Σ_{pq} J_pq Z_p Z_q  (connectivity cost — prefer connected regions)
```

Where c_q encodes the quality of physical qubit q (derived from calibration: T1, T2, readout fidelity, recent error rates). The measurement outcome is a qubit region mask.

**What gets built:**
- `quasi-solvayeur/src/qubit_map.rs` — Virtual-to-physical qubit mapping
- HAL Contract extension: per-qubit calibration data (`GET /hal/backends/{name}/calibration`)
- Qubit region scoring: quality × connectivity × contiguity
- ATW Hamiltonian term for qubit allocation (appended to backend selection)
- Integration: Afana's output QASM gets physical qubit indices from the kernel, not from the backend transpiler

**Prerequisite:** HAL Contract must expose per-qubit calibration. Currently it only exposes device-wide averages.

**Analogy:** Virtual memory → physical RAM mapping. The user program sees virtual qubits 0..n. The kernel maps them to physical qubits based on current hardware state.

---

## Phase C — Multi-Program Spatial Scheduling

**What the kernel decides:** Multiple programs arrive concurrently. The kernel partitions the QPU into disjoint regions and assigns each program to a region.

**Why it matters:** A 156-qubit Heron processor running a 20-qubit program wastes 136 qubits. Three 20-qubit programs could run simultaneously on disjoint qubit sets — 3× throughput, same hardware cost. This is the quantum equivalent of multi-core scheduling.

**Hamiltonian extension:**
```
H_C = H_B + Σ_{programs i,j} Σ_{qubits q} J_ij · n_iq · n_jq  (collision penalty)
```

Where n_iq = 1 if program i uses qubit q. Programs that would overlap on the same physical qubits are anti-correlated. The measurement outcome gives a spatial partition.

**What gets built:**
- `quasi-solvayeur/src/partition.rs` — QPU spatial partitioning
- Program queue with priority levels
- Collision detection: two programs cannot share physical qubits unless they are explicitly entangled (Phase E)
- Throughput tracking: how many programs per second the kernel dispatches

**Prerequisite:** Phase B (qubit mapping must exist before multi-program mapping).

**Analogy:** Multi-core process scheduling. Each program gets a "core" (qubit region). The kernel ensures no two programs share a core unless explicitly designed to.

---

## Phase D — Temporal Scheduling and Coherence Budgeting

**What the kernel decides:** When each program runs, for how long, and whether it needs to be split into smaller pieces to fit within the coherence window.

**Why it matters:** T2 = 150μs is a hard deadline. A circuit with depth 500 and gate time 0.5μs needs 250μs — it won't run faithfully on a superconducting QPU. The kernel must either:
1. Route to a trapped-ion backend (T2 ~ 500μs) — Phase A already does this.
2. Split the circuit into sub-circuits that each fit within T2, run them separately, and stitch the results classically (circuit knitting).
3. Run it on Huoma if entanglement permits.

Option 2 is the new capability. The kernel becomes time-aware.

**Hamiltonian extension:**
```
H_D = H_C + Σ_slices Σ_q T_sq Z_sq   (time slot allocation)
           + λ · penalty(depth > T2)     (coherence violation)
```

**What gets built:**
- `quasi-solvayeur/src/timeslice.rs` — Circuit splitting into coherence-compatible slices
- Coherence budget tracker: remaining T2 per qubit after each gate layer
- Circuit knitting interface: split a circuit at low-entanglement cuts, run sub-circuits, recombine classically
- Integration with Afana's noise analysis (already estimates circuit time)

**Prerequisite:** Phase B (qubit mapping), Phase C (spatial scheduling). Circuit knitting is a research-level capability — may start with a simple "reject if too deep" policy and evolve.

**Analogy:** Real-time OS scheduling with hard deadlines. Each quantum process has a deadline (T2). The kernel ensures completion within the deadline or rejects/splits the job.

---

## Phase E — Entanglement Tracking

**What the kernel decides:** Which qubits are entangled with which, across which programs, and what the consequences are for scheduling.

**Why it matters:** Entanglement is shared quantum state — the analog of shared memory between processes. Two sub-computations that share entangled qubits cannot be measured independently without destroying the shared state. The kernel must:
1. Track which qubits are entangled (entanglement map)
2. Ensure entangled qubits are co-scheduled (same QPU, overlapping time window)
3. Detect when entanglement is consumed (measurement collapses it)
4. Prevent scheduling conflicts (two programs measuring the same entangled pair)

**What gets built:**
- `quasi-solvayeur/src/entanglement.rs` — Entanglement map: graph of entangled qubit pairs with creation/consumption timestamps
- Dependency tracking: program B depends on the entangled output of program A → must be sequenced
- Mid-circuit measurement awareness: measurement of qubit q collapses entanglement with qubit p → kernel must update the entanglement map
- Integration with ZX-IR: the ZX graph structure encodes entanglement at compile time; the kernel uses this for scheduling

**Prerequisite:** Phase D (temporal scheduling must exist before entanglement-aware scheduling).

**Analogy:** Shared memory management + IPC. Entangled qubits are shared memory segments. The kernel tracks which processes have access and prevents concurrent mutation (measurement).

---

## Phase F — Error Budget Management

**What the kernel decides:** How much error each computation has accumulated, when to trigger correction, and how to allocate correction resources.

**Why it matters:** Every gate introduces error. The kernel tracks the cumulative error budget for each running program. When the error exceeds a threshold, the kernel can:
1. Trigger a quantum error correction cycle (using ancilla qubits)
2. Re-run the computation on a higher-fidelity backend
3. Terminate the program with an error report
4. Checkpoint and restart from a known state

**What gets built:**
- `quasi-solvayeur/src/error_budget.rs` — Per-program error accumulator
- Error threshold policies: warn at 5% total error, halt at 10%
- Correction scheduling: allocate ancilla qubits for error correction cycles
- Integration with Afana's noise analysis and HAL Contract gate error data

**Prerequisite:** Phase E (entanglement tracking — error correction requires understanding the entanglement structure to apply the right code).

**Analogy:** Page fault handling + resource limits (cgroups). The kernel monitors error accumulation like the OS monitors memory usage, and intervenes when limits are exceeded.

---

## Phase G — The Full Kernel

All phases integrated. The Solvayeur manages:

```
User program (Ehrenfest CBOR)
        │
   Afana compiles → ZX-IR → QASM
        │
   Solvayeur Kernel
   ├── Backend selection (Phase A)
   ├── Qubit region allocation (Phase B)
   ├── Multi-program spatial scheduling (Phase C)
   ├── Temporal scheduling + coherence budgeting (Phase D)
   ├── Entanglement tracking (Phase E)
   └── Error budget management (Phase F)
        │
   H_kernel encodes ALL decisions simultaneously
   ATW measurement → complete resource allocation plan
        │
   Execute on QPU / Huoma / hybrid
```

The scheduling Hamiltonian at Phase G has qubits encoding:
- Which backend (⌈log₂ backends⌉ qubits)
- Which qubit region (⌈log₂ regions⌉ qubits per program)
- Which time slot (⌈log₂ slots⌉ qubits per program)
- Entanglement constraints (coupling terms)
- Error budget penalties (bias terms)

One measurement. One allocation plan. All decisions simultaneous.

The kernel is still an Ehrenfest program. It still compiles through Afana. It still measures on a QPU (or Huoma, when the scheduling landscape is simple). The OS still compiles itself.

---

## What changes, what doesn't

**Stays the same across all phases:**
- ATW algorithm (compile → measure → dispatch → observe → update)
- Ehrenfest encoding of the scheduling problem
- Afana compilation of the kernel
- Self-referential property (kernel decides where kernel runs)
- Bias learning from outcomes

**Changes across phases:**
- Number of qubits in the scheduling Hamiltonian (grows with capability)
- Number of terms in the Hamiltonian (richer constraints)
- What "dispatch" means (backend → backend + qubits → backend + qubits + time + entanglement + error budget)
- Complexity of the observation/reward signal

---

## Dependency graph

```
A (backend selection)     ← DONE
│
B (qubit region)          ← needs per-qubit HAL calibration
│
C (multi-program)         ← needs qubit mapping
│
D (temporal / coherence)  ← needs spatial scheduling + circuit knitting research
│
E (entanglement)          ← needs temporal scheduling
│
F (error budget)          ← needs entanglement tracking
│
G (full kernel)           ← all integrated
```

Each phase is independently useful. Phase B alone improves circuit fidelity. Phase C alone improves throughput. They compose but don't require all-or-nothing.

---

*The OS compiles itself. At every phase.*
