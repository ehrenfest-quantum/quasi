# QUASI — System Overview
**2026-04-11 · Post-Solvayeur**

---

## What QUASI Is

QUASI is a quantum operating system. Not a framework, not a SDK, not a job queue. An operating system — software that manages heterogeneous compute resources (CPU, GPU, QPU) and makes allocation decisions autonomously, so the user never sees the infrastructure.

The user submits a physics problem. QUASI decides what runs where, when, and how. The result comes back verified against a classical reference.

---

## What QUASI Can Do Today

### Compile quantum programs
An Ehrenfest program (a Hamiltonian + evolution time + noise constraints + observables, encoded as CBOR binary) enters the Afana compiler and comes out as executable OpenQASM 2.0 or 3.0. The compilation pipeline:

```
CBOR → deserialize → EhrenfestProgram
  → Trotterize (1st/2nd order Suzuki decomposition)
  → EhrenfestAst (typed gate-level circuit)
  → Type check (qubit bounds, gate arity, parameter binding)
  → Lower to ZX-IR (every gate becomes ZX calculus spiders)
  → ZX-IR validation (structural, phase consistency)
  → Noise analysis (depth, fidelity estimate, T1/T2 compliance)
  → Observable measurement synthesis (SZ/SX/E basis rotations)
  → QASM emission (V2 or V3)
  → Optimize (T-gate reduction + ZX-calculus simplification via QuiZX)
```

Afana is a Rust-only compiler. 13 modules, 3500 lines, 208 tests. It handles Hamiltonians with arbitrary Pauli terms on arbitrary qubit counts. The output runs on any QASM-compatible backend.

### Schedule across 14 backends
The quasi-scheduler takes a compiled circuit and decides which backend should execute it. Filter-Score-Bind pipeline (Kubernetes pattern):

1. **Filter** — hard constraints: does the backend support the required gates? enough qubits? within noise budget?
2. **Score** — weighted ranking: gate set fit (0.25), noise margin (0.20), connectivity (0.20), queue depth (0.10), cost (0.10), backend preference (0.10), cache hit (0.05)
3. **Bind** — highest score wins

14 backend profiles with real hardware specs:

| Backend | Type | Qubits | Native Gates | Topology |
|---|---|---|---|---|
| IBM Heron/Torino/Eagle/Marrakesh | Superconducting | 127-156 | RZ/SX/X/ECR | Heavy-hex |
| IQM Garnet/Sirius | Superconducting | 6-20 | PRX/CZ | Grid/Star |
| IonQ Aria/Forte | Trapped ion | 25-36 | GPI/GPI2/MS/ZZ | All-to-all |
| Quantinuum H1/H2 | Trapped ion | 20-56 | RZ/RX/RY/ZZ | All-to-all |
| Rigetti Ankaa-3 | Superconducting | 84 | RZ/RX/iSWAP | Grid |
| AQT Pine | Trapped ion | 24 | RZ/RXX/R | All-to-all |
| Huoma ProjectedTTN | Tensor network | 1,000,000 | All | All-to-all |
| Statevector simulator | Exact | 30 | All | All-to-all |

### Profile workloads for classical/quantum routing
The Huoma profiler bridge estimates whether a circuit is classically simulable before spending QPU time:

- **Bond dimension < threshold** → run on Huoma (free, instant, exact for structured circuits)
- **Exponential entanglement** → route to QPU (the circuit genuinely needs quantum hardware)
- **Gray zone** → let other scheduling plugins decide

With Huoma at 1,000,000 qubits in 5.2 seconds (ProjectedTTN, PR hiq-lab/huoma#14), almost everything stays classical. QPU is reserved for provable quantum advantage.

### Cache results (skip redundant execution)
Content-addressed cache using BLAKE3 hashing with calibration version baked into the key (Nix model):

```
key = BLAKE3(circuit_cbor ‖ backend_id ‖ sorted_parameters ‖ calibration_version)
```

- Cache hit → return stored result instantly, skip QPU entirely
- Changed calibration → different key → no invalidation logic needed
- L1: in-memory HashMap (fast lookup)
- L2: filesystem JSON (survives restarts)
- Stale entries evict naturally under capacity pressure

### Run on real quantum hardware
Verified end-to-end on IBM quantum processors:

- **IBM Torino** (156q Heron): Rabi oscillation 1q, Job d7cui865nvhs73a53h70
- **IBM Strasbourg** (127q Eagle): Transverse Ising 2q, 10 Trotter steps, 94 gates, Job d7cuqjp4p4gc73f5o63g — 70.7% ground state, consistent with Afana's 0.763 fidelity estimate

### Make scheduling decisions on a QPU (Solvayeur)
The Solvayeur is the QUASI kernel. It is itself a quantum program — an Ehrenfest program compiled by Afana — that runs on a QPU and whose measurement outcomes are dispatching decisions.

**ATW algorithm** (Around The World):

The scheduling Hamiltonian on n = ⌈log₂ m⌉ qubits for m backends:

```
H(k) = Σ_ij J_ij Z_i Z_j  +  Σ_i h_i(k) Z_i  +  Γ Σ_i X_i
        (contention)          (learned bias)      (exploration)
```

One ATW round:
1. **COMPILE** — Afana compiles H(k) to QASM3
2. **EVOLVE** — Trotterized time evolution
3. **MEASURE** — bitstring → backend index
4. **DISPATCH** — route workload to selected backend
5. **OBSERVE** — reward r(k) = f(fidelity, latency, cost)
6. **UPDATE** — h_i(k+1) = (1-λ)·h_i(k) + η·r(k)·(-1)^{b(k)[i]}

The bias fields learn from experience. Exploration (Γ) anneals over time. The ground state of H(k) converges toward the optimal resource allocation.

**Verified on real QPU:** 10 ATW rounds on IBM Strasbourg, 2-qubit scheduling circuit, 100 shots per round. The kernel explored 4 backends (Huoma, IBM, IQM, Quantinuum) through genuine quantum measurement and began converging toward Huoma as optimal.

**Self-referential property:** The Solvayeur decides whether its own scheduling circuit runs on Huoma (classical, when the allocation landscape is simple) or on QPU (quantum, when it's complex). The OS compiles itself through the same pipeline it dispatches user programs to.

### Govern itself (Senate Loop)
The quasi-senate is an AI governance daemon that continuously improves the codebase:

- **5-role pipeline**: Council → Drafter → Gate → Solver → Reviewer
- **Open-weight models**: qwen3.6-plus, qwen3-coder-next, kimi-k2, cogito, deepseek-r1
- **Runs on Camelot** via systemd timers (draft every 2h, solve every 2h)
- **Current output**: ~3 approved PRs/day, 7 open PRs pending review

The senate now targets Phase 2: scheduler plugins, cache upgrades, Solvayeur refinement, and compiler hardening.

---

## Architecture

```
User submits Ehrenfest CBOR
        │
   ┌────┴────────────────────────────┐
   │         QUASI OS                │
   │                                 │
   │  ┌───────────┐ ┌────────────┐  │
   │  │   Afana    │ │  Cache     │  │
   │  │  Compiler  │ │  (BLAKE3)  │  │
   │  │  CBOR→QASM │ │  hit→skip │  │
   │  └─────┬─────┘ └─────┬──────┘  │
   │        │              │         │
   │  ┌─────┴──────────────┴──────┐  │
   │  │      SOLVAYEUR            │  │
   │  │  H(k) = ZZ + Z + X       │  │
   │  │  compiled by Afana        │  │
   │  │  measurement = decision   │  │
   │  └────────────┬──────────────┘  │
   │               │                 │
   │  ┌────────────┴──────────────┐  │
   │  │      Scheduler            │  │
   │  │  filter → score → bind    │  │
   │  │  14 backend profiles      │  │
   │  │  Huoma profiler bridge    │  │
   │  └────────────┬──────────────┘  │
   └───────────────┼─────────────────┘
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
  Huoma        IBM/IQM/       Quantinuum/
  1M qubits    Rigetti        IonQ/AQT
  5.2 seconds  Heavy-hex      All-to-all
  (classical)  (supercond.)   (trapped ion)
```

---

## Crate Map

| Crate | Lines | Tests | Purpose |
|---|---|---|---|
| afana | 3,500 | 208 | Ehrenfest compiler: CBOR → AST → ZX-IR → QASM |
| quasi-scheduler | 1,599 | 54 | Filter-Score-Bind scheduler + 14 backend profiles + Huoma profiler |
| quasi-cache | 614 | 17 | BLAKE3 content-addressed result cache |
| quasi-solvayeur | 986 | 27 | ATW quantum kernel (the OS) |
| quasi-demo | 900 | — | Pipeline demo + VQE orchestrator + Solvayeur demo |
| quasi-senate | ~5,000 | 10 | AI governance daemon |
| **Total** | **~12,600** | **375** | |

All Rust. Zero Python in the critical path. Zero vendor SDKs.

---

## What Makes This Novel

No existing quantum computing system does what QUASI does:

| | QOS (Berkeley) | HALO (UCLA) | IBM QCSC | **QUASI** |
|---|---|---|---|---|
| Scheduling kernel | Classical | Classical | Classical (Slurm) | **QPU-native (Solvayeur)** |
| Scheduling decisions | if-then-else | Algorithm | SPANK plugin | **Quantum measurement** |
| Self-compiling | No | No | No | **Yes (Afana compiles Solvayeur)** |
| User program format | Qiskit circuits | Qiskit circuits | OpenQASM | **Ehrenfest (physics-native CBOR)** |
| Kernel program format | N/A | N/A | N/A | **Ehrenfest (same as user programs)** |
| Classical reference | None | None | None | **Huoma (1M qubits, 5.2s)** |
| Hardware-agnostic | IBM only | IBM only | IBM + PASQAL | **14 backends, 6 vendors** |
| AI self-governance | No | No | No | **Senate Loop (open-weight models)** |

---

## What Comes Next

**Phase 2** (senate is now targeting this):
- Scheduler plugins: connectivity scoring, latency prediction, live HAL status
- Cache: moka L1 (W-TinyLFU), redb L2 (embedded ACID), circuit canonicalization
- Solvayeur: real contention coupling, adaptive learning rate, QPU-in-the-loop
- Afana: CCX lowering, per-gate noise models, SY observable

**Phase 3** (6-12 months, from Parliament Resolution):
- QUASI Cache operational across production workloads
- Autonomous routing end-to-end (sinC² integrated into Arvak → Solvayeur)
- QOBLIB proof: ≥3 benchmark classes, ≥100 qubits, measurable advantage
- Q-Level 2 declared

---

*The OS compiles itself. Around the world.*
