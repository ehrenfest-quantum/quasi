# QUASI as a Harness

*One-pager — 2026-08-01*

## What it is

QUASI is an **agent harness whose benchmark and whose product are the same artefact**. Most
harnesses run models against a fixed task set and report a score. QUASI runs models against a
live quantum-computing OS project: the tasks are generated from the project's own state, the
solutions are merged into it, and the project's progress *is* the score. There is no held-out
test set, because there is no set — there is a codebase that has to keep working.

The measurement engine is the **Senate Loop**, a five-role pipeline over a roster of
open-weight models:

```
A1 Council  → charter: current phase, frontier level, goal for the next batch
A2 Drafter  → drafts one issue against that charter
A3 Gate     → accepts or rejects the draft
   ─────────────────────────────────────────────────────────
B1 Solver   → produces a patch for an open issue
B2 Reviewer → judges the patch before it becomes a PR
```

Every LLM call is written to Postgres telemetry (26 measurement points across 7 quality
dimensions — JSON compliance, latency, retries, verdict reasoning, provider fidelity). Merged
outcomes are appended to a hash-linked, tamper-evident ledger.

## What makes it an unusual harness

**1. Contamination resistance is structural, not procedural.**
The language being written — *Ehrenfest* — is a **CBOR-encoded binary format with no text form
and no file extension**. A human never reads an Ehrenfest program. There is no corpus, no Stack
Overflow, no GitHub full of examples. A model cannot have memorised it, and cannot pattern-match
its way through. Most benchmarks resist contamination by holding data back; this one resists it
by asking for code in a language that did not exist before the project created it.

**2. The oracle is physics, not opinion.**
Above the scaffolding levels, success is defined by quantities measurable on real hardware: gate
counts, circuit depth, decoherence budgets, Bell-state fidelity. A compiler pass that emits valid
QASM3 with 10× redundant gates *fails* even with green CI. This closes the usual escape hatch
where a model produces something that type-checks and satisfies a grader without doing the work.

**3. The task ladder is traversal-gated.**
Five capability levels, L0 scaffolding → L1 language foundations → L2 compiler construction →
L3 hardware backends → L4 Turing-complete quantum programming. A model is "activated" at a level
only after **6 CI-passing completions with no human edits** (the *Planck Quota*), verifiable from
public commit history. The ceiling is open: L3–L4 are currently beyond any known model.

*On the name: the Pauli-Test is named after **Wolfgang Pauli the critic**, not the Exclusion
Principle. Pauli was physics' most meticulous and least diplomatic tester of ideas — the man who
dismissed sloppy work as "not even wrong" and whom colleagues called the conscience of physics.
That is precisely the standard this harness applies: a result is not accepted because it looks
plausible, compiles, or passes a grader. It is accepted when it survives scrutiny that was trying
to reject it. (`docs/pauli-test-audit.md` reads the name as a claim about the Exclusion Principle
and argues the analogy fails; that critique addresses a meaning the name was never intended to
carry.)*

**4. It writes a compiler for the language it is being tested in.**
The work under measurement is `afana` — a Rust compiler taking CBOR → typed AST → type check →
ZX-calculus IR → OpenQASM, with noise constraints enforced as *compile-time type errors*. The
harness is therefore measuring compiler construction for a language with no prior art, where the
correctness criterion is downstream physical behaviour on a QPU.

**5. It is a component of an operating system, not an application.**
Below the compiler sits the **HAL Contract** — "the POSIX of QPUs" — and above it the intent that
the OS kernel is *itself* an Ehrenfest program, where measurement outcomes are dispatching
decisions. Tasks therefore inherit OS-level constraints: hardware abstraction must not leak into
the compiler, and vendor SDKs are architecturally forbidden inside `afana`. Several architectural
invariants are enforced by CI, not by convention.

**6. Constrained model pool by policy.**
Open-weight models only — no closed commercial APIs — with rotation across ~9 providers, fair
assignment by usage count, provider diversity to prevent collusion between drafter and reviewer,
and cost-tier preference so a free local model is used when its host is reachable.

## Verification posture

The loop is deliberately asymmetric about what blocks:

| Check | In the autonomous loop | In CI |
|---|---|---|
| `cargo test --workspace --all-targets` | **blocking** | blocking |
| `cargo clippy -- -D warnings` | advisory (fed back as signal) | **blocking** |

A solver cannot fix a pre-existing lint in an unrelated crate, so a toolchain bump would
otherwise block every unrelated task; a human seeing the same failure in CI can. Patches are also
verified against the **merged** state, not the branch's own base — green-on-a-stale-base is the
classic way a broken change looks healthy.

## Generation is human-gated

The loop **proposes**; it does not publish. Approved drafts are submitted as ActivityPub
`quasi:Propose` activities to the board, which screens them (trivial rejected; small scope
requires ≥2 components or ≥3 success criteria; near-duplicate titles rejected; open L0 proposals
capped) and holds them pending. Only a human acceptance turns a proposal into a GitHub issue.
This exists because an ungated generator produced formulaic near-duplicate issues at scale — the
failure mode was in the *task supply*, not the solver.

## Current state (2026-08-01)

Deployed and verified end-to-end: draft → proposal → human accept → issue. Solver pool spans 9
models across 6 providers. **The timers are disabled** — the loop runs only when invoked
manually. The ledger currently reports `valid: false` from a single lost entry caused by an
unlocked read-modify-write, tracked separately.
