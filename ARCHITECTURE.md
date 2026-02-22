# QUASI Architecture

## The Stack

```
Natural language (human describes problem)
        ↓
   AI model (Claude, GPT, Llama, ...)
        ↓  generates
   Ehrenfest program (.ef)       ← physics-native, not human-readable
        ↓  compiled by
   Afana                         ← the Ehrenfest compiler
        ↓  optimized via
   ZX-calculus
        ↓  extracts gate sequences
   HAL Contract (L0)             ← the POSIX of QPUs
        ↓
   IBM | IQM | Quantinuum | neQxt | Simulator | ...
```

## Layer Model

| Layer | Name | Status | License |
|-------|------|--------|---------|
| L0 | HAL Contract Specification | ✅ v2.2 | Apache 2.0 |
| L1 | Hardware Adapters | ✅ IBM, IQM, Scaleway, AQT | Apache 2.0 |
| L2 | Afana Compiler / IR | ✅ arvak-compile, arvak-ir | Apache 2.0 |
| L3 | QUASI Runtime Services | 🔲 Specified, not built | AGPL v3 |
| L4 | QUASI Standard Interface | 🔲 Spec in progress | Apache 2.0 |
| L5 | Application Libraries (Urns) | 🔲 Community grows this | Various |
| — | ZX-calculus | ✅ PyZX (MIT, external) | MIT |
| — | Ehrenfest Language | 🔲 Concept complete | AGPL v3 |

## Ehrenfest

Named after Paul Ehrenfest (1880–1933), whose theorem bridges quantum expectation values to classical equations of motion — exactly what the language does.

**Key design decisions:**
- **Not human-readable** — CBOR binary format, no canonical text form
- **Physics-native** — Hamiltonians, observables, evolution times — not gates
- **Noise-as-type-system** — exceeding T2 is a compile-time type error, not runtime
- **AI-primary** — optimized for LLM generation, not human authoring

The human never sees an Ehrenfest program. The full loop:

```
human intent → AI → Ehrenfest → Afana → QPU → result → AI → human
```

Community shortname: **Paul**. As in: "write me a Paul program", "that's valid Paul."

## Afana

Named after Tatiana Afanasyeva (1876–1964), Paul Ehrenfest's wife and mathematical collaborator. She co-authored the Urnenmodell, contributed foundational work on statistical mechanics, and made Ehrenfest's physical intuitions mathematically rigorous.

**Afana is the Ehrenfest compiler.** It takes `.ef` CBOR programs, applies ZX-calculus optimization, and emits HAL Contract gate sequences for execution on any QUASI-compatible backend.

The naming is accurate: Afana turns Ehrenfest's representations into something that executes.

## Urns

Named after Ehrenfest's Urnenmodell — a probabilistic diffusion model co-developed with Tatiana Afanasyeva.

**Urns are QUASI's package format** — what crates are in Rust, what packages are in npm. An urn is a reusable, composable quantum computation unit: a typed Ehrenfest program with declared observables, noise requirements, and HAL Contract dependencies.

```
urn publish --name grover-search --version 0.1.0
urn add grover-search
```

The urnery is the public urn registry.

## QUASI Standard Interface (L4) — Sketch

```rust
// Observable-oriented, derived from Ehrenfest types
trait QuantumContext {
    fn submit(&self, program: &EhrenfestProgram, shots: u32) -> JobHandle;
    fn await_result<O: Observable>(&self, job: JobHandle) -> O::Output;
    fn capabilities(&self) -> PhysicalContext;
}

// Noise context — no POSIX equivalent, QUASI-unique
trait NoiseContext {
    fn current_t1(&self) -> Duration;
    fn current_t2(&self) -> Duration;
    fn cooling_profile(&self) -> Option<CoolingProfile>;
    fn quiet_window_now(&self) -> bool;
}

// Provenance — Alsvid hardware attestation
trait ProvenanceContext {
    fn attest(&self) -> ProvenanceCertificate;
    fn verify(&self, cert: &ProvenanceCertificate) -> bool;
}
```

The L4 interface is derived from Ehrenfest's type system, not defined independently.

## The Coherence Principle

The project structure mirrors the OS structure mirrors the language structure:

| QUASI OS | QUASI Project |
|----------|---------------|
| L3 Job Scheduler | Public Task Board |
| QPU Backend executes circuit | AI Agent executes task |
| Formal type checker | CI / Spec Validator |
| Provenance Certificate | Attribution Ledger |
| Ehrenfest job unit | Contribution (typed change-set) |

Full argument: [docs/coherence.md](docs/coherence.md)

## Positioning vs. existing standards

| Standard | Relationship |
|----------|-------------|
| QIR (Microsoft) | Afana compiles to HAL-compatible gate sequences equivalent to QIR output |
| OpenQASM 3 | Possible Afana output target, not intermediate format |
| ZX-calculus | Optimization layer between Ehrenfest and gate sequences |
| PennyLane | PennyLane has Hamiltonian support as a Python layer; Ehrenfest is a separate representation level |

## Further reading

- [HAL Contract v2.2](https://github.com/hiq-lab/arvak) — the foundation, fully implemented
- Ehrenfest concept paper — complete, will be published
- QUASI Governance — in progress
