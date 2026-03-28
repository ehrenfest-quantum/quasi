# Architecture Comparison: Quasi vs IBM QCSC

> Comparison of the Quasi stack against IBM's "Reference Architecture of a Quantum-Centric Supercomputer" (Seelam et al., arXiv:2603.10970, March 2026).

---

## Paper Summary

IBM proposes **Quantum-Centric Supercomputing (QCSC)** — a three-phase roadmap for integrating QPUs into HPC clusters alongside GPUs and CPUs. Key abstractions:

- **Tensor Compute Graph (TCG)** — unified DAG mixing quantum circuits and classical compute
- **Quantum Systems API (QSA)** — programmatic boundary between software and QPU hardware
- **QRMI** — Slurm plugin exposing QPUs as generic HPC resources
- **Three coupling modes** — batch, near-time (iterative), real-time (microsecond QEC feedback)

The architecture spans 19 pages, targets chemistry/materials science, and is authored by IBM Quantum leadership (Gambetta, Chow, Sheldon, et al.).

---

## Layer-by-Layer Comparison

### Compilation

| | IBM QCSC | Quasi |
|---|---|---|
| **Language** | Python (Qiskit SDK) | Rust (Afana) |
| **Input format** | Qiskit circuit objects | Ehrenfest CBOR binary |
| **Output format** | Vendor-specific ISA (unspecified IR) | OpenQASM 2.0 / 3.0 |
| **Noise handling** | Runtime mitigation (TEM, Pauli propagation) | Compile-time rejection (type-level noise constraints) |
| **Dependencies** | Qiskit, NumPy, SciPy, vendor transpiler plugins | `ciborium`, `quizx`, `serde`, `clap` — no Python, no vendor SDK |
| **Deployment** | Python environment + pip packages | Single static Rust binary |

**Key difference:** IBM mitigates noise *after* execution. Afana rejects programs that violate their noise budget *before* compilation completes. These approaches are complementary — HAL drivers could still apply runtime mitigation — but compile-time rejection is cheaper and eliminates wasted QPU time on doomed circuits.

**Key difference:** IBM's compilation depends on Qiskit, a Python framework with heavy transitive dependencies. Afana is a zero-dependency Rust binary. In HPC environments (which QCSC explicitly targets), dependency-free deployment matters.

### Hardware Abstraction

| | IBM QCSC | Quasi |
|---|---|---|
| **Abstraction name** | Quantum Systems API (QSA) | HAL Contract |
| **Interface** | Unspecified ("potentially vendor-portable") | `POST /hal/jobs` (REST, vendor-neutral) |
| **Vendor neutrality** | Aspirational — early deployments use IBM proprietary interconnects | Enforced — compiler cannot address hardware directly |
| **Calibration data** | Flows into Qiskit for qubit selection | Lives in `hal-drivers/<vendor>/`, exposed via HAL Contract |
| **Enforcement** | Architectural intent | CI boundary check (`compiler-boundary` job) |

**Key difference:** QSA is described as "potentially vendor-portable." HAL Contract is vendor-portable by construction — there is no code path from Afana to any hardware API. This is enforced by CI, not by convention.

```
IBM:    Qiskit → (vendor transpiler) → QPU
Quasi:  Ehrenfest (CBOR) → Afana (Rust) → OpenQASM → HAL Contract → HAL driver → hardware
```

The extra indirection through HAL Contract means adding a new vendor requires only a new HAL driver, not compiler changes.

### Task Orchestration

| | IBM QCSC | Quasi |
|---|---|---|
| **Scheduler** | Slurm + QRMI plugin | ActivityPub (federated) |
| **Model** | Centralized job queue | Decentralized activity stream |
| **Job lifecycle** | Slurm job states | `quasi:Propose` → `quasi:Claim` → `quasi:Complete` |
| **Multi-site** | Slurm federation (complex) | ActivityPub federation (native) |
| **Audit trail** | Slurm accounting DB | Immutable ActivityPub ledger |

**Key difference:** QCSC bolts QPUs onto Slurm, which is a centralized scheduler designed for homogeneous compute nodes. Quasi uses ActivityPub — a W3C-standard federation protocol — which scales naturally across institutional boundaries without a central coordinator. For the multi-institution quantum HPC that QCSC Phase 3 envisions, federated orchestration is architecturally simpler than Slurm federation.

### Workflow Representation

| | IBM QCSC | Quasi |
|---|---|---|
| **Abstraction** | Tensor Compute Graph (TCG) | Ehrenfest program (CBOR) |
| **Scope** | Full hybrid workflow (quantum + classical compute) | Physics-level problem specification |
| **Granularity** | Operation-level DAG | Hamiltonians, observables, noise constraints |
| **Classical compute** | First-class TCG nodes | Outside scope (classical compute lives in quasi-board workflows) |

**Key difference:** TCG operates at the circuit/operation level — it describes *how* to compute. Ehrenfest operates at the physics level — it describes *what* to compute. Afana derives the circuit from the physics specification. This means Quasi programs are hardware-independent at a deeper level: the same Ehrenfest program compiles to different circuits for different backends without user intervention.

### Error Handling

| | IBM QCSC | Quasi |
|---|---|---|
| **Strategy** | Runtime error mitigation | Compile-time noise rejection + optional runtime mitigation |
| **Techniques** | TEM, Pauli propagation, hierarchical QEC | Type-level noise budget in Ehrenfest spec |
| **When errors caught** | After execution | Before compilation (noise budget) or after execution (HAL driver) |
| **QEC** | Hierarchical: inner codes (FPGA), outer codes (GPU) | Delegated to HAL drivers |

IBM's hierarchical QEC design (inner FPGA decoding + outer GPU decoding) is well-engineered. Quasi currently delegates QEC entirely to HAL drivers, which is architecturally correct but means the HAL Contract API may need to surface QEC metadata (syndrome data, logical error rates) as backends mature.

---

## Where IBM QCSC Is Ahead

### Coupling Modes
IBM defines three coupling modes with clear latency requirements:

- **Batch** — loose coupling, independent submission (SQD workflows)
- **Near-time** — iterative feedback loops (closed-loop SQD, VQE)
- **Real-time** — microsecond synchronization (QEC research, dynamic circuits)

Quasi's HAL Contract currently supports only batch mode (`POST /hal/jobs` → poll for results). Near-time and real-time coupling are not yet addressed. As algorithms like VQE or adaptive circuits become practical, HAL Contract will need to support iterative and low-latency modes.

### GPU Co-Processing
QCSC treats GPUs as first-class compute alongside QPUs. Tensor network error mitigation (TEM) and outer QEC decoding run on co-located GPUs. Quasi's architecture has no explicit GPU compute layer — classical compute is handled externally by quasi-board workflows.

### Calibration Integration
IBM's paper stresses that real-time calibration data must flow into the compiler for optimal qubit selection. Quasi's architecture correctly places calibration in HAL drivers, but the HAL Contract API does not yet expose calibration metadata to inform compilation decisions.

---

## Where Quasi Is Ahead

### Vendor Neutrality (Enforced, Not Aspirational)
IBM's QSA is "potentially vendor-portable." Quasi's HAL Contract is the *only* path to hardware, enforced by CI. There is no way to bypass it — no vendor SDK imports in the compiler, no direct hardware API calls. This is a structural guarantee, not a design goal.

### Compile-Time Noise Rejection
Ehrenfest programs carry noise constraints as type-level metadata. If a program's noise budget is infeasible for the target backend, Afana refuses to emit QASM. This eliminates wasted QPU time on circuits that would produce meaningless results — a problem IBM addresses only *after* execution with statistical mitigation.

### Dependency-Free Deployment
Afana is a single Rust binary with no runtime dependencies. Deploying it to an HPC node requires copying one file. IBM's stack requires Python, Qiskit, NumPy, SciPy, and vendor-specific transpiler plugins — a significant operational burden in locked-down HPC environments.

### Federated Task Management
ActivityPub federation is a W3C standard with mature implementations. Slurm federation is complex, fragile, and designed for tightly-coupled clusters. For the multi-institution quantum computing that QCSC Phase 3 envisions, Quasi's approach is architecturally simpler.

### Physics-Level Abstraction
Ehrenfest programs describe physics (Hamiltonians, observables, noise constraints), not circuits. The compiler derives the optimal circuit for each backend. IBM's TCG operates at the circuit level, meaning users must understand hardware-specific circuit construction even when using high-level Qiskit abstractions.

---

## Gaps This Paper Exposes in Quasi

1. **Near-time / real-time coupling** — HAL Contract needs a feedback mode for iterative algorithms (VQE, QAOA) and potentially a streaming mode for QEC research
2. **Calibration metadata in HAL Contract** — HAL drivers know about calibration, but there's no API for the compilation layer to query backend fidelity data for topology-aware routing
3. **GPU compute layer** — no explicit support for GPU-accelerated classical post-processing (error mitigation, tensor network contractions)
4. **Workflow DAG** — Ehrenfest captures the quantum problem, but there's no equivalent of TCG for expressing hybrid classical-quantum workflows as a single executable graph

---

## Strategic Assessment

This paper validates the problem space Quasi operates in. IBM's conclusion — that quantum computing needs a real systems architecture, not ad-hoc scripting — aligns exactly with Quasi's thesis.

However, IBM's solution is shaped by IBM's constraints:
- **Qiskit lock-in** — the compilation stack depends on their SDK
- **Centralized scheduling** — Slurm reflects their existing HPC business
- **Runtime mitigation** — compensates for noisy hardware they ship today
- **"Potentially portable"** — vendor neutrality deferred to future phases

Quasi's architecture already enforces what QCSC only proposes. The structural decisions — Rust compiler, HAL Contract boundary, ActivityPub orchestration, compile-time noise rejection — are not incremental improvements over IBM's approach. They are fundamentally different design choices that become more valuable as quantum computing scales beyond single-vendor, single-site deployments.

---

## References

- Seelam et al., "Reference Architecture of a Quantum-Centric Supercomputer," arXiv:2603.10970, March 2026
- [[coherence|Quasi Coherence Model]]
- [[optimization|Afana Optimization Pipeline]]
