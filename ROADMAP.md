# QUASI Roadmap — Paul's Boutique Tracklist

> 15 phases. Named after the Paul's Boutique tracklist in order.
> The arc: from the first Ehrenfest spec to a Turing-complete quantum OS.

---

## Phase 1 — To All the Girls
> Publish the canonical Ehrenfest v0.1 specification: a complete CDDL schema, CBOR binary format definition, and three physics-native example programs.

**What gets built**
- `spec/ehrenfest-v0.1.cddl` — CDDL schema defining all top-level Ehrenfest constructs: `program`, `hamiltonian`, `observable`, `parameter`, `system`
- `spec/FORMAT.md` — binary encoding rules: CBOR tag assignments, field ordering, canonical serialization
- `examples/heisenberg.paul` — Heisenberg spin chain Hamiltonian in CBOR-encoded Ehrenfest format
- `examples/rabi.paul` — Rabi oscillation model (two-level system, drive term)
- `examples/ising.paul` — transverse-field Ising model with configurable chain length
- `spec/LANGUAGE.md` — prose description of the language semantics: what a `hamiltonian` means, what an `observable` means, what the compiler is expected to do with them

**Success criteria**
- `cddl validate spec/ehrenfest-v0.1.cddl` exits 0 (schema is self-consistent)
- All three `.paul` example files decode without error using any conformant CBOR library
- Each example file round-trips: encode → decode → re-encode produces identical bytes
- `spec/LANGUAGE.md` states explicitly that programs express Hamiltonians and observables, not gate sequences
- Schema covers at minimum: Pauli operators (X, Y, Z, I), tensor products, linear combinations with complex coefficients, parameterized terms, and observable definitions

**Dependencies**
- None

**Scope:** Small

---

## Phase 2 — Shake Your Rump
> Extend Ehrenfest to v0.2 with a complete operator algebra, making the Hamiltonian representation sufficient to express any finite-dimensional quantum system.

**What gets built**
- `spec/ehrenfest-v0.2.cddl` — schema extended with: fermionic operators (creation/annihilation), bosonic operators (with truncation bound), time-dependent Hamiltonians (piecewise-constant schedules), composite systems (tensor product of subsystems), and sum-of-terms representation normalized to Pauli decomposition
- `spec/CHANGELOG.md` — diff from v0.1 to v0.2, with rationale for each addition
- `examples/vqe-h2.paul` — H2 molecule Hamiltonian in second quantization, Pauli-decomposed (Jordan-Wigner mapping result expressed directly as Pauli sum)
- `examples/qaoa-maxcut.paul` — QAOA cost Hamiltonian for MaxCut on a 4-node graph
- `examples/driven-qubit.paul` — time-dependent Rabi drive with piecewise-constant schedule
- `spec/OPERATORS.md` — full operator reference: every operator type in v0.2, its CBOR encoding, and its physical interpretation

**Success criteria**
- `cddl validate spec/ehrenfest-v0.2.cddl` exits 0
- All five example files (three from Phase 1 + two new) validate against v0.2 schema
- v0.1 examples are forward-compatible: they decode without modification under v0.2 schema
- `spec/OPERATORS.md` enumerates every CBOR tag introduced in v0.2 with no ambiguity in encoding
- H2 Hamiltonian example encodes a minimum of 4 Pauli terms (correct for Jordan-Wigner H2 at minimal basis)
- Schema supports at least 64-qubit systems without structural changes (qubit count is a parameter, not a fixed enum)

**Dependencies**
- Phase 1 (To All the Girls): v0.1 CDDL schema and CBOR format definition must exist as the base to extend

**Scope:** Medium

---

## Phase 3 — Johnny Ryall
> Build the Ehrenfest parser: a CBOR deserializer that reads `.paul` files, validates them against the v0.2 schema, and produces a typed in-memory parse tree.

**What gets built**
- `afana/src/parser/mod.rs` — top-level parser module
- `afana/src/parser/cbor.rs` — CBOR deserializer consuming raw bytes, producing untyped CBOR value tree
- `afana/src/parser/schema.rs` — schema validator: walks CBOR value tree against v0.2 CDDL rules, returns typed `EhrenfestProgram` struct or structured error
- `afana/src/ast/mod.rs` — AST definition: `EhrenfestProgram`, `Hamiltonian`, `Term`, `Operator`, `Observable`, `Parameter`, `SystemSpec` as Rust structs/enums
- `afana/tests/parser/` — test suite: one test file per example program from Phases 1 and 2, plus negative tests for malformed CBOR and schema violations
- `afana/src/parser/error.rs` — structured parse error type with byte offset, expected vs. found, and schema path

**Success criteria**
- All five example `.paul` files from Phases 1 and 2 parse to `EhrenfestProgram` without error
- Malformed CBOR (truncated bytes, wrong major type) returns a structured error with byte offset, not a panic
- Schema violations (missing required field, out-of-range value, unknown operator tag) return errors referencing the CDDL rule name
- `cargo test afana::parser` passes with zero failures
- Parse round-trip: serialize `EhrenfestProgram` back to CBOR and compare with original — identical bytes for all five examples
- Parser handles programs up to 1,000 Hamiltonian terms without stack overflow (iterative traversal, not recursive)

**Dependencies**
- Phase 1 (To All the Girls): `.paul` example files used as test fixtures
- Phase 2 (Shake Your Rump): v0.2 schema is the validation target; v0.2 operator types must be defined before the AST can represent them

**Scope:** Medium

---

## Phase 4 — Egg Man
> Bootstrap the Afana compiler project: establish the build system, define the internal IR data structures, wire CBOR input through the parser to a stub output, and emit the first (trivially incorrect but structurally valid) QASM3 file.

**What gets built**
- `afana/Cargo.toml` — workspace manifest with crates: `afana-parser`, `afana-ir`, `afana-backend`, `afana-cli`
- `afana/src/ir/mod.rs` — internal IR definition: `IrProgram`, `IrHamiltonian`, `IrTerm`, `IrQubitRef` — a flattened, index-based representation of the Ehrenfest parse tree, independent of CBOR encoding
- `afana/src/lowering/ehrenfest_to_ir.rs` — lowering pass: `EhrenfestProgram` → `IrProgram`; resolves parameter references, expands tensor products, normalizes all terms to Pauli-sum form
- `afana/src/backend/qasm3_stub.rs` — stub QASM3 emitter: for each qubit referenced in `IrProgram`, emits `qubit[n] q; h q[0]; // stub` and closes with `OPENQASM 3.0;` header — structurally valid QASM3, semantically meaningless
- `afana/src/cli/main.rs` — CLI entry point: `afana compile <input.paul> --backend qasm3 --output <out.qasm>`
- `afana/tests/integration/stub_compile.rs` — integration test: compile each Phase 1 and 2 example, assert output file exists, assert it begins with `OPENQASM 3.0;`, assert qubit count matches system size in Ehrenfest source

**Success criteria**
- `cargo build --release` produces a working `afana` binary
- `afana compile examples/heisenberg.paul --backend qasm3 --output out.qasm` exits 0 and writes a file
- Output file begins with `OPENQASM 3.0;` and declares the correct number of qubits for each example
- `cargo test` passes for all unit tests in `afana-parser` and `afana-ir`
- Integration test suite (5 examples) passes: all produce structurally valid QASM3
- `IrProgram` for H2 VQE example contains exactly the number of Pauli terms present in the Ehrenfest source

**Dependencies**
- Phase 3 (Johnny Ryall): the parser and `EhrenfestProgram` AST are the input to the lowering pass

**Scope:** Medium

---

## Phase 5 — High Plains Drifter
> Define the ZX-IR intermediate representation and implement the Ehrenfest-to-ZX-IR lowering pass, replacing the stub IR with a graph-based structure that encodes quantum semantics as ZX-calculus spiders and wires.

**What gets built**
- `afana/src/zxir/mod.rs` — ZX-IR graph definition: `ZxGraph` as adjacency list of `ZxNode` (Z-spider, X-spider, H-box, input boundary, output boundary) and `ZxEdge` (regular wire, Hadamard edge); nodes carry phases as exact rational multiples of π stored as `(numerator: i64, denominator: u64)`
- `afana/src/zxir/phase.rs` — phase arithmetic: addition mod 2π, negation, scalar multiplication — exact rational arithmetic, no floating point
- `afana/src/lowering/ir_to_zxir.rs` — lowering pass: `IrProgram` → `ZxGraph`; maps each Pauli term in the Hamiltonian to a ZX gadget
- `afana/src/zxir/validation.rs` — structural validator: checks that every boundary has exactly one wire, no dangling edges, phase values in [0, 2π)
- `afana/tests/zxir/` — test suite: unit tests for lowering of single Pauli terms (X, Y, Z, ZZ, ZZZ), plus full-program tests for all five Ehrenfest examples
- `spec/ZXIR.md` — specification of the ZX-IR format: node types, edge types, phase encoding, and mapping rules from Ehrenfest Hamiltonian terms to ZX gadgets

**Success criteria**
- `IrProgram` for a single-qubit Z Hamiltonian (H = Z) lowers to a ZX graph with exactly one Z-spider with phase π
- `IrProgram` for a two-qubit ZZ interaction lowers to the standard ZX gadget for ZZ
- `zxir::validation::validate` returns `Ok` for all graphs produced from Phase 1 and 2 examples
- Phase arithmetic is exact: `π/2 + π/2 = π` represented as `(1,2) + (1,2) = (1,1)` with no floating-point intermediate
- `cargo test afana::zxir` passes with zero failures
- `spec/ZXIR.md` is complete enough that an independent implementer could write a conformant lowering pass from it alone

**Dependencies**
- Phase 4 (Egg Man): `IrProgram` and the Afana build system must exist

**Scope:** Large

---

## Phase 6 — The Sounds of Science
> ZX-calculus rewriting rules are implemented in Afana, enabling graph-based simplification before gate synthesis.

**What gets built**
- `afana/src/zx/rewrite.rs` — rewriting engine with spider fusion, identity removal, and Hadamard cancellation rules
- `afana/src/zx/spider.rs` — Z-spider and X-spider node types with phase labels
- `afana/src/zx/graph.rs` — ZX-diagram as a multigraph
- `spec/zx-ir-v0.1.md` — formal definition of ZX-IR node types, wire types, and rewrite axioms
- `tests/zx/` — unit test suite covering each rewrite rule with before/after diagram assertions
- `benches/zx_rewrite.rs` — benchmark measuring rewrite pass wall time on Heisenberg, Rabi, and Ising examples

**Success criteria**
- Spider fusion: two adjacent same-color spiders merge into one; test asserts node count reduction
- Identity removal: a phase-0 spider with exactly two wires is eliminated; test asserts wire count reduction
- Hadamard cancellation: two consecutive H-boxes on the same wire cancel; test asserts removal
- All three Phase 1 examples pass through the rewrite engine without panic
- Rewrite pass terminates on all test inputs (step counter with hard cap)
- Benchmark baseline recorded in `benches/baseline-zx.txt` for CI regression tracking

**Dependencies**
- Phase 5 (High Plains Drifter): ZX-IR definition and Ehrenfest → ZX-IR lowering must be complete

**Scope:** Medium

---

## Phase 7 — 3-Minute Rule
> Afana produces its first valid QASM3 output from a ZX-IR diagram, and CI begins tracking gate count as a build metric.

**What gets built**
- `afana/src/codegen/qasm3.rs` — emitter that walks a post-rewrite ZX-IR diagram and outputs QASM3 text
- `afana/src/codegen/gate_set.rs` — mapping from ZX spider phases to universal gate set (RZ, SX, X, CX)
- `afana/src/codegen/decompose.rs` — Euler-angle decomposition for single-qubit rotations from spider phases
- `tests/codegen/` — end-to-end tests: Ehrenfest CBOR in, QASM3 text out, validated by `openqasm` Python package in CI
- `ci/gate-count.sh` — CI script that compiles each example program, counts gate operations, writes counts to `ci/artifacts/gate-counts.json`
- `.github/workflows/gate-count.yml` — CI job that fails if gate count for any example regresses by more than 5%

**Success criteria**
- `afana compile heisenberg.paul` produces syntactically valid QASM3 accepted by the `openqasm` reference parser with zero errors
- Output QASM3 for the Rabi example contains only gates from the universal gate set (RZ, SX, X, CX)
- Gate count baseline committed to `ci/artifacts/gate-counts.json`; CI gate-count job passes on main branch
- End-to-end test for all three Phase 1 example programs passes in CI
- Compilation of any Phase 1 example completes in under 5 seconds on CI runner

**Dependencies**
- Phase 6 (The Sounds of Science): rewritten ZX-IR diagrams are the input to codegen
- Phase 5 (High Plains Drifter): ZX-IR lowering provides input to Phase 6

**Scope:** Large

---

## Phase 8 — Hey Ladies
> Afana gains a type checker that tracks qubit identity and entanglement through the full Ehrenfest → QASM3 pipeline.

**What gets built**
- `afana/src/typeck/mod.rs` — type checker entry point, invoked after parsing and before lowering
- `afana/src/typeck/qubit_env.rs` — qubit environment: maps Ehrenfest qubit identifiers to type states (fresh, entangled, measured, discarded)
- `afana/src/typeck/entanglement.rs` — entanglement type lattice: `Separable`, `Entangled(set)`, `Mixed`; propagated through two-qubit operators
- `afana/src/typeck/errors.rs` — typed error enum: use-after-measure, double-discard, entanglement violation, dimension mismatch
- `spec/type-system-v0.1.md` — specification of the qubit type system, the entanglement lattice, and the error taxonomy
- `tests/typeck/` — test suite with valid and invalid Ehrenfest programs; invalid programs must produce the expected typed error

**Success criteria**
- A program that applies a unitary to a qubit after measuring it is rejected with a `use-after-measure` error
- A two-qubit program applying CNOT is assigned `Entangled({q0, q1})` type after the gate
- A program with mismatched Hilbert space dimensions is rejected with a `dimension_mismatch` error
- All three Phase 1 example programs pass the type checker without errors
- Type checker adds less than 50 ms to end-to-end compilation time for all existing examples

**Dependencies**
- Phase 3 (Johnny Ryall): parse tree and CBOR deserializer are the type checker's input
- Phase 7 (3-Minute Rule): end-to-end pipeline must exist for type checker integration

**Scope:** Large

---

## Phase 9 — 5-Piece Chicken Dinner
> Afana becomes hardware-aware: it reads a backend topology descriptor and maps ZX-IR to the native gate set and qubit connectivity of a target device.

**What gets built**
- `spec/backend-v0.1.cddl` — CDDL schema for a backend descriptor: qubit count, coupling map, native gate set, T1/T2 times
- `backends/ibm_torino.json` — reference descriptor for IBM Torino (133 qubits, Heron heavy-hex topology, native: RZ/SX/X/CX)
- `backends/iqm_garnet.json` — reference descriptor for IQM Garnet (20 qubits, crystal-20 topology, native: PRX/CZ)
- `afana/src/backend/mod.rs` — backend loader: deserializes a backend descriptor JSON into a typed `Backend` struct
- `afana/src/backend/routing.rs` — SWAP-based qubit routing: maps logical to physical qubits satisfying coupling-map constraints
- `afana/src/backend/native_gates.rs` — gate rebase pass: rewrites universal gate set output to target backend's native gate set
- `tests/backend/` — routing tests asserting two-qubit operations only appear on coupled qubit pairs for each reference backend

**Success criteria**
- Heisenberg example with `--backend backends/ibm_torino.json` produces QASM3 where every CX acts on a coupled pair; CI test asserts this
- Same example with `--backend backends/iqm_garnet.json` produces QASM3 using only PRX and CZ gates; no CX in output
- SWAP overhead logged to `ci/artifacts/swap-counts.json` for baseline tracking
- Backend descriptor with an undefined native gate name is rejected at load time
- Routing completes in under 10 seconds for programs using at most 20 logical qubits

**Dependencies**
- Phase 7 (3-Minute Rule): universal-gate QASM3 output and gate set abstraction are input to the native gate rebase pass
- Phase 6 (The Sounds of Science): ZX-IR graph structure used by the routing pass

**Scope:** Large

---

## Phase 10 — Looking Down the Barrel of a Gun
> Afana integrates a noise-aware compilation mode that tracks decoherence budgets and rejects or warns when a program's expected gate error exceeds a configurable threshold.

**What gets built**
- `spec/noise-model-v0.1.cddl` — CDDL schema extension to the backend descriptor: per-gate error rates, T1/T2 relaxation times, readout error per qubit
- `backends/ibm_torino.json` — extended with noise fields (T1, T2, single-qubit gate error, two-qubit gate error, readout error)
- `afana/src/noise/budget.rs` — decoherence budget tracker: walks compiled gate sequence, accumulates expected circuit error using `1 - ∏(1 - eᵢ)` approximation
- `afana/src/noise/report.rs` — noise report struct: per-qubit decoherence contribution, dominant error source, total estimated fidelity; serialized to JSON
- `afana/src/noise/threshold.rs` — threshold enforcer: reads `--fidelity-floor` CLI flag (default 0.5); warns below threshold, errors below 0.1
- `tests/noise/` — test suite: deep circuit (100+ two-qubit gates) produces fidelity estimate below 0.9; Bell circuit produces estimate above 0.95
- `ci/artifacts/fidelity-estimates.json` — CI artifact recording fidelity estimates for all example programs per backend

**Success criteria**
- Heisenberg example with `--noise-report` produces a well-formed JSON noise report with fidelity in (0, 1]
- A synthetic 200-gate test circuit compiled to IBM Torino produces fidelity estimate below 0.5
- A Bell-state program (2 gates) compiled to IBM Torino produces fidelity estimate above 0.95
- `--fidelity-floor 0.8` causes non-zero exit status when compiling the 200-gate circuit
- A backend descriptor with a negative gate error rate is rejected at load time

**Dependencies**
- Phase 9 (5-Piece Chicken Dinner): backend descriptor format and compiled, routed QASM3 gate sequence
- Phase 7 (3-Minute Rule): gate count and gate type information from QASM3 emitter

**Scope:** Large

---

## Phase 11 — Car Thief
> Full ZX-calculus optimization pass ships, achieving a gate reduction ratio of at least 10% on all benchmark circuits.

**What gets built**
- `afana/passes/zx_optimize.rs` — multi-pass ZX-calculus rewriter: spider fusion, identity elimination, local complementation, and pivot rewriting
- `afana/passes/gate_count.rs` — benchmark harness: records gate counts before and after optimization, fails CI if reduction ratio falls below 10%
- `spec/benchmarks/` — canonical benchmark suite: Bell, GHZ-8, Grover-4, Ising-8 circuits as Ehrenfest CBOR
- `tests/optimization/` — property-based tests verifying optimized circuits are semantically equivalent to originals (unitary matrix comparison up to global phase)
- CI job `optimize-bench` — runs gate_count harness on every PR, reports per-circuit reduction ratios as job summary

**Success criteria**
- Gate reduction ratio ≥ 10% on all five benchmark circuits (Bell, GHZ-8, Grover-4, Ising-8, Heisenberg-4)
- Zero semantic regressions: unitary fidelity of optimized vs. unoptimized circuits ≥ 1 − 1e-9 on all benchmarks
- `optimize-bench` CI job is mandatory and blocking
- No regressions in QASM3 output validity (Phase 7 `qasm3-validate` job continues to pass)

**Dependencies**
- Phase 5 (High Plains Drifter): ZX-IR definition and Ehrenfest → ZX-IR lowering
- Phase 6 (The Sounds of Science): ZX-calculus rewriting primitives
- Phase 7 (3-Minute Rule): gate synthesis pipeline and CI gate count metric baseline

**Scope:** Large

---

## Phase 12 — What Comes Around
> Ehrenfest gains variational parameter support and Afana compiles parametric programs, enabling VQE and QAOA workflows.

**What gets built**
- `spec/ehrenfest-v0.3.cddl` — extends v0.2 schema with `param` declarations: named floating-point symbols appearing inside Hamiltonian coefficients and rotation angles
- `examples/vqe_h2.paul` — hydrogen VQE ansatz as parametric Ehrenfest program
- `examples/qaoa_maxcut.paul` — QAOA MaxCut on a 6-node graph as parametric Ehrenfest program
- `afana/frontend/param.rs` — parameter table: collects all declared params, assigns bind-time slots, validates no unbound symbols reach gate synthesis
- `afana/codegen/qasm3_param.rs` — emits QASM3 `input` declarations and parametric gate calls (`rx(theta)`, `rz(phi)`)
- `tests/parametric/` — round-trip tests: parse parametric Ehrenfest → compile → bind concrete values → simulate → compare expectation values against reference

**Success criteria**
- Both example programs compile end-to-end to valid parametric QASM3 with no unbound symbols
- Given a concrete parameter map, compiled output is equivalent to a non-parametric program with values substituted at source
- Round-trip tests pass: simulated expectation values for H2 VQE at optimal parameters match reference (< 1 mHartree error)
- Parametric programs survive the Phase 11 optimization pass without binding parameters prematurely (optimizer treats param slots as opaque scalars)

**Dependencies**
- Phase 3 (Johnny Ryall): CBOR deserializer and schema validation to parse extended v0.3 schema
- Phase 7 (3-Minute Rule): gate synthesis pipeline (parametric gate emission extends this)
- Phase 8 (Hey Ladies): type system (a param in a qubit position is a type error)
- Phase 11 (Car Thief): optimization pass must handle parametric gates without collapsing free variables

**Scope:** Medium

---

## Phase 13 — Shadrach
> Classical control flow enters Ehrenfest: conditionals, loops, and mid-circuit measurement are first-class language constructs.

**What gets built**
- `spec/ehrenfest-v0.4.cddl` — adds `measure`, `cond`, and `repeat` nodes; mid-circuit measurement result is a typed classical bit bound to a name
- `afana/frontend/control.rs` — parses and validates control flow nodes; enforces no-cloning constraint (a measured qubit's quantum type is consumed), detects unreachable branches
- `afana/mid/cfg.rs` — control-flow graph (CFG) builder: converts Ehrenfest control flow into a basic-block CFG for downstream passes
- `afana/codegen/qasm3_control.rs` — emits QASM3 `if`, `for`, and `reset` statements; mid-circuit measurement emits `measure q -> c` inline
- `examples/teleportation.paul` — quantum teleportation using mid-circuit Bell measurement and classical feedforward
- `tests/control/` — test suite: branch taken vs. not taken, loop unrolling for static bounds, mid-circuit measurement invalidating qubit type in subsequent expressions

**Success criteria**
- `teleportation.paul` compiles to valid QASM3 containing mid-circuit measurement and classically conditioned corrections
- Type checker rejects programs that use a qubit after measurement (consumed type)
- Static loop bounds are unrolled at compile time; dynamic bounds emit QASM3 `for` loops
- CFG pass does not regress Phase 11 optimization: gate reduction ratio on non-control-flow benchmarks unchanged
- All Phase 12 parametric examples continue to compile

**Dependencies**
- Phase 3 (Johnny Ryall): parser infrastructure extended to handle new AST node types
- Phase 8 (Hey Ladies): type system must track qubit consumption post-measurement
- Phase 11 (Car Thief): optimization pass must be CFG-aware (cannot fuse across measurement barriers)
- Phase 12 (What Comes Around): parametric programs must compose with control flow

**Scope:** Large

---

## Phase 14 — Ask for Janice
> The full Ehrenfest memory model ships: explicit qubit allocation and deallocation, a scoped runtime, and a complete v1.0 language specification.

**What gets built**
- `spec/ehrenfest-v1.0.cddl` — final normative schema: merges all prior CDDL versions (v0.1–v0.4), adds `alloc`, `free`, and `scope` nodes, freezes the format as v1.0
- `spec/ehrenfest-v1.0.md` — prose specification: defines semantics for every node type, memory model (qubit lifetimes, scope rules, aliasing prohibition), and normative encoding rules
- `afana/runtime/allocator.rs` — qubit allocator: maps logical Ehrenfest qubits to physical indices respecting hardware topology, enforces no double-free and no use-after-free at compile time
- `afana/runtime/scope.rs` — lexical scope tracker: qubit names only valid within their enclosing `scope` block; crossing scope boundaries is a compile error
- `afana/passes/lifetime.rs` — lifetime analysis pass: annotates each qubit with allocation, last-use, and free points; feeds the allocator and topology mapper
- `tests/memory/` — test suite: scope escape detection, double-free detection, use-after-free detection, large-circuit allocation (100+ qubits on Heron topology)
- `spec/conformance/` — conformance test corpus: 30+ Ehrenfest programs covering the full language surface; any compliant Afana implementation must pass all of them

**Success criteria**
- `afana compile` accepts every program in `spec/conformance/` and rejects every `expect-error` program with the correct error code
- Lifetime analysis catches all use-after-free and double-free errors at compile time (zero runtime panics on conformance corpus)
- Allocated qubits are released before QASM3 output terminates (no qubit leaks detectable by static analysis)
- Ehrenfest v1.0 spec is self-contained: an independent implementer can write a conformant parser and type checker from it alone
- All prior phase test suites pass (no regressions across Phases 3–13)

**Dependencies**
- Phase 8 (Hey Ladies): type system (qubit types underpin the ownership and lifetime model)
- Phase 9 (5-Piece Chicken Dinner): hardware topology awareness (allocator must respect connectivity constraints)
- Phase 13 (Shadrach): control flow CFG (lifetime analysis must traverse CFG branches to compute conservative live ranges)

**Scope:** Large

---

## Phase 15 — B-Boy Bouillabaisse
> QUASI becomes a real quantum OS: Shor's algorithm compiles end-to-end through the full Ehrenfest + Afana stack, all CI levels pass, and the system is demonstrably Turing-complete.

**What gets built**
- `examples/shor_15.paul` — Shor's algorithm for N=15 as a native Ehrenfest program: modular exponentiation Hamiltonian, QFT subroutine, mid-circuit measurement, classical post-processing loop, parametric phase kickback — exercises every language feature from Phases 1–14
- `afana/passes/pipeline.rs` — unified compilation pipeline: single entry point sequencing all passes (parse → validate → type-check → CFG → lifetime → ZX-IR → ZX-optimize → backend → QASM3), with pass-level timing and gate-count reporting
- `spec/turing-completeness.md` — formal argument that the Ehrenfest v1.0 + Afana pipeline is Turing-complete: classical control flow + unbounded memory model + universal gate set (H, T, CNOT) reachable from arbitrary Hamiltonians
- `ci/levels.yaml` — defines all mandatory CI levels: `schema-validate`, `parse-roundtrip`, `qasm3-validate`, `optimize-bench`, `parametric-roundtrip`, `control-flow`, `memory-safety`, `conformance`, `shor-e2e`
- `tests/e2e/shor.rs` — end-to-end test: compiles `shor_15.paul`, runs QASM3 against the `qasm-simulator` backend, verifies factors {3, 5} returned with probability ≥ 0.9 across 1024 shots
- `CHANGELOG.md` — documents every breaking schema change from v0.1 through v1.0 with migration notes

**Success criteria**
- `shor_15.paul` compiles without errors through the full Afana pipeline in under 60 seconds on a commodity laptop
- End-to-end simulation returns correct factors (3 and 5) with probability ≥ 0.9 over 1024 shots
- All 9 CI levels in `ci/levels.yaml` pass on main with zero suppressions
- Gate reduction on the Shor circuit is ≥ 10% (Phase 11 criterion holds at scale)
- Conformance corpus from Phase 14 passes in full
- `spec/turing-completeness.md` demonstrates a concrete encoding of an arbitrary Turing machine transition function as an Ehrenfest program
- Compilation pipeline produces deterministic output: identical input CBOR produces bit-for-bit identical QASM3 across platforms and runs

**Dependencies**
- Phase 12 (What Comes Around): parametric programs (QFT phase angles are variational parameters)
- Phase 13 (Shadrach): classical control flow (Shor's period-finding loop and measurement feedforward)
- Phase 14 (Ask for Janice): full memory model and Ehrenfest v1.0 spec (Shor requires dynamic qubit allocation across subroutine calls)
- All prior phases: the pipeline.rs pass sequencer depends on every pass having a stable, tested interface

**Scope:** Large
