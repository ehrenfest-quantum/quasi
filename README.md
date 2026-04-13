# QUASI — Quantum OS

[![CI](https://github.com/ehrenfest-quantum/quasi/actions/workflows/ci.yml/badge.svg)](https://github.com/ehrenfest-quantum/quasi/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE-AGPL-3.0)
[![Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-1f6feb)](LICENSE-APACHE-2.0)
[![GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-0a7b83)](LICENSE-GPL-3.0)
[![Docs](https://img.shields.io/badge/docs-reference-blue)](docs/)

[![Ledger](https://img.shields.io/badge/quasi--ledger-live-brightgreen)](https://gawain.valiant-quantum.com/quasi-board/ledger)
[![Dashboard](https://img.shields.io/badge/dashboard-live-brightgreen)](https://quasi.hal-contract.org/stats/dashboards)
[![Issues](https://img.shields.io/github/issues/ehrenfest-quantum/quasi)](https://github.com/ehrenfest-quantum/quasi/issues)

**A quantum operating system that schedules workloads across CPU, GPU, and QPU — with a kernel that runs on quantum hardware.**

QUASI doesn't just submit jobs to quantum computers. It decides what runs where, using quantum measurement as the scheduling mechanism. The user submits a physics problem. QUASI compiles it, profiles it, caches results, and routes it to the optimal backend — all invisibly.

---

## What QUASI Does

```
User submits Ehrenfest program (CBOR binary)
        |
   Afana Compiler
   CBOR -> type check -> ZX-IR -> noise analysis -> QASM3
        |
   Solvayeur Kernel (ATW algorithm)
   Scheduling Hamiltonian compiled by Afana
   QPU measurement = dispatching decision
        |
   +----+--------+----------+
   v              v          v
 Huoma          IBM/IQM    Quantinuum/
 1M qubits      Rigetti    IonQ/AQT
 5.2 seconds    (superc.)  (trapped ion)
        |
   Cache (BLAKE3, Nix model)
   Same circuit + same calibration = instant result
        |
   Result to user
```

### Verified on real quantum hardware

- **IBM Strasbourg** (127q Eagle): Transverse Ising 2q, 10 Trotter steps, Job `d7cuqjp4p4gc73f5o63g`
- **Solvayeur ATW kernel**: 10 scheduling rounds on IBM Strasbourg, each dispatching decision was a real quantum measurement

---

## Architecture

| Crate | Lines | Tests | What it does |
|---|---|---|---|
| **afana** | 8,576 | 208 | Ehrenfest compiler: CBOR -> AST -> ZX-IR -> QASM |
| **quasi-scheduler** | 2,052 | 54 | Filter-Score-Bind scheduler + 14 backend profiles |
| **quasi-solvayeur** | 994 | 27 | ATW quantum kernel (the OS) |
| **quasi-demo** | 1,119 | -- | Pipeline demo + VQE orchestrator + Solvayeur demo |
| **quasi-cache** | 614 | 17 | BLAKE3 content-addressed result cache |
| **quasi-senate** | 7,345 | 10 | AI governance daemon (open-weight models) |
| **quasi-board** | 3,884 | -- | ActivityPub task ledger |
| **Total** | **32,000+** | **375** | |

All Rust in the critical path. Zero vendor SDKs. 14 backend profiles (IBM, IQM, IonQ, Quantinuum, Rigetti, AQT, simulators).

---

## The Solvayeur

The Solvayeur is the QUASI kernel. It is itself a quantum program — an Ehrenfest program compiled by Afana — whose measurement outcomes are resource dispatching decisions.

**ATW algorithm** (Around The World):

```
H(k) = Sum_ij J_ij Z_i Z_j  +  Sum_i h_i(k) Z_i  +  Gamma Sum_i X_i
        (contention)            (learned bias)        (exploration)
```

Each round: compile H(k) through Afana -> Trotterize -> measure -> bitstring = backend index -> dispatch workload -> observe reward -> update bias. The ground state of H(k) converges toward the optimal resource allocation. Exploration anneals over time.

**Self-referential**: the Solvayeur decides whether its own scheduling circuit runs on Huoma (classical) or QPU (quantum). The OS compiles itself.

No existing system — not QOS (Berkeley), not HALO (UCLA), not IBM QCSC — uses a QPU as the scheduling kernel. The Solvayeur is the first.

---

## Quickstart

### Compile an Ehrenfest program

```bash
# Build the compiler
cargo build -p afana --release

# Compile a Heisenberg model to QASM3
cat spec/examples/heisenberg_4q.cbor.hex | xxd -r -p > /tmp/h4.cbor
./target/release/afana /tmp/h4.cbor --qasm v3 --optimize --stats
```

### Run the end-to-end demo

```bash
# Classical pipeline (no QPU needed)
cargo run -p quasi-demo --release -- spec/examples/transverse_ising_2q.cbor.hex --skip-qpu

# Solvayeur kernel demo (mock backends)
cargo run -p quasi-demo --bin solvayeur-demo --release -- --rounds 40 --backends 4
```

### Browse Ehrenfest examples

```bash
ls spec/examples/*.md
```

10 example programs: Rabi oscillation, Ising model, Heisenberg chain, GHZ state, QAOA MaxCut, Hubbard model, H2 molecule, spin chain (16q), VQE parametric, and the Solvayeur scheduling Hamiltonian itself.

---

## Ehrenfest

**Ehrenfest** is QUASI's specification language. CBOR binary, no text form, no file extension. Programs describe Hamiltonians and observables — not gates. The compiler chooses gates. The program expresses physics.

Named after Paul Ehrenfest (1880-1933). The Ehrenfest theorem bridges classical and quantum mechanics: expectation values of quantum observables follow classical equations of motion. The Solvayeur bridges classical and quantum computing: measurement outcomes of the scheduling Hamiltonian drive classical resource allocation.

**Afana** is the Ehrenfest compiler. Named after Tatiana Afanasyeva, Ehrenfest's wife and mathematical collaborator.

**Solvayeur** is the quantum kernel. Named after the Solvay Conferences where Ehrenfest presented alongside Bohr, Einstein, and Heisenberg.

---

## Roadmap (Paul's Boutique Tracklist)

15 phases. 11 complete. Named after the Beastie Boys album.

| # | Phase | Status |
|---|---|---|
| 1 | To All the Girls — Ehrenfest v0.1 spec | DONE |
| 2 | Shake Your Rump — Operator algebra | DONE (90%) |
| 3 | Johnny Ryall — CBOR parser | DONE |
| 4 | Egg Man — Afana bootstrap | DONE |
| 5 | High Plains Drifter — ZX-IR + lowering | DONE |
| 6 | Sounds of Science — ZX rewriting (QuiZX) | DONE |
| 7 | 3-Minute Rule — QASM3 output | DONE |
| 8 | Hey Ladies — Type checker | DONE |
| 9 | 5-Piece Chicken Dinner — Hardware-aware compilation | DONE |
| 10 | Looking Down the Barrel — Noise-aware compilation | DONE |
| 11 | Car Thief — ZX optimization >=10% | DONE |
| 12 | What Comes Around — Variational parameters | DONE |
| 13 | Shadrach — Classical control flow | IN PROGRESS (60%) |
| 14 | Ask for Janice — Memory model, v1.0 spec | NOT STARTED |
| 15 | B-Boy Bouillabaisse — Quantum OS | IN PROGRESS (40%) |

[Live dashboard](https://quasi.hal-contract.org/stats/d/quasi-project-progress/quasi-project-progress)

---

## Senate Loop

The codebase is continuously improved by an AI governance daemon running open-weight models (qwen3.6-plus, gemma-4, nemotron-ultra, kimi-k2, cogito, deepseek-r1). The senate drafts issues, solves them, reviews solutions, and opens PRs — autonomously.

5-role pipeline: Council -> Drafter -> Gate -> Solver -> Reviewer

[Model leaderboard](https://quasi.hal-contract.org/stats/d/quasi-leaderboard/quasi-leaderboard-e28094-model-standings)

---

## Contributing

### Setup

```bash
git clone https://github.com/ehrenfest-quantum/quasi.git
cd quasi
cargo test --workspace   # 375 tests
```

### What's needed

| Skill | Where |
|---|---|
| **Rust** | Afana compiler, scheduler, cache, Solvayeur |
| **Quantum physics** | Ehrenfest examples, spec review |
| **Systems / scheduling** | quasi-scheduler plugins, backend profiles |
| **Distributed systems** | quasi-board (ActivityPub), federation |

### For AI agents

```bash
claude mcp add quasi npx -y tsx ./quasi-mcp/src/index.ts
```

Tools: `list_tasks` -> `claim_task` -> implement -> `complete_task`

---

## License

| Component | License | Rationale |
|---|---|---|
| Ehrenfest spec, HAL Contract | [Apache 2.0](LICENSE-APACHE-2.0) | Specs must be free for anyone to implement |
| quasi-agent, quasi-mcp, Afana | [GPL v3](LICENSE-GPL-3.0) | Distributed tools — copyleft |
| quasi-board | [AGPL v3](LICENSE-AGPL-3.0) | Network service — closes SaaS loophole |

---

## Who

QUASI is initiated by Daniel Hinderink. It is an open project — the goal is a neutral foundation once the community is established.

---

> *"He was not merely the best teacher in our profession whom I have ever known; he was also passionately preoccupied with the development and destiny of men, especially his students."*
>
> -- Albert Einstein, eulogy for Paul Ehrenfest, 1933

*The OS compiles itself. Around the world.*
