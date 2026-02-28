# QUASI-Grand-Prix: QuantCert Collaboration Brief

This document packages the non-code deliverables requested for `#86` into a
single reviewable artifact: a formal specification outline, a Coq-style proof
sketch, an Afana integration design, a collaboration proposal draft, and a
race-evaluation rubric.

## 1. Contextuality-Preserving Compilation

### Goal

Afana should detect when compilation destroys contextual structure that was
present in the source Ehrenfest program. In this issue, "contextuality" is not
an executable theorem prover yet; it is a compiler-observable invariant with a
well-defined approximation.

### Source-side abstraction

Treat an Ehrenfest program as inducing a finite measurement-context hypergraph:

- vertices: observables or Pauli constraints that appear in the program
- hyperedges: commuting measurement contexts
- annotations: sign/parity constraints induced by Hamiltonian terms and
  observable declarations

Define the source contextuality witness:

`Ctx(P) = { (C_i, parity_i) }`

where each `C_i` is a commuting context and `parity_i` is the parity relation
that must hold if the program expresses a Kochen-Specker style obstruction.

### Post-compilation abstraction

After Afana emits a circuit, derive a compiled witness:

`Ctx'(K) = { (C'_j, parity'_j) }`

from the circuit's effective measurement contexts after gate synthesis,
rewriting, routing, and backend lowering.

### Preservation criterion

Compilation is contextuality-preserving if every source witness maps to a
compiled witness that is at least as strong under a chosen embedding `phi`:

`Preserve(P, K) := forall w in Ctx(P), exists w' in Ctx'(K), phi(w, w')`

The minimum useful `phi` for a first implementation is:

- support preservation: the same logical qubit set is still represented
- parity preservation: the contradiction/parity relation is unchanged
- rank floor: contextuality rank does not decrease below a configured threshold

### Practical metric

For a first Afana pass, use a scalar heuristic:

- `contextuality_rank_source`
- `contextuality_rank_compiled`
- `contextuality_slack = contextuality_rank_compiled - contextuality_rank_source`

Failure if:

- `contextuality_slack < 0`, or
- a required witness disappears entirely

This is intentionally conservative. It turns an abstract research question into
a compiler signal that can be surfaced in CI, logs, or optimization reports.

## 2. Afana IR Pass Design

### Proposed pass

`contextuality_check`

### Position in pipeline

1. Parse / build logical circuit IR
2. Run algebraic optimization passes
3. Run `contextuality_check` before and after optimization
4. Emit backend-specific circuit

The pass should be usable in two modes:

- advisory: annotate warnings, never block compilation
- strict: fail compilation when preservation is violated

### Proposed interface

```python
@dataclass(frozen=True)
class ContextualityWitness:
    logical_qubits: tuple[int, ...]
    context_count: int
    parity_signature: tuple[int, ...]
    rank: int


@dataclass(frozen=True)
class ContextualityReport:
    source: tuple[ContextualityWitness, ...]
    compiled: tuple[ContextualityWitness, ...]
    preserved: bool
    rank_delta: int
    dropped_witnesses: tuple[ContextualityWitness, ...]
    message: str


def contextuality_check(
    source_program: EhrenfestProgram,
    compiled_ir: ZXIR,
    *,
    mode: Literal["advisory", "strict"] = "advisory",
) -> ContextualityReport:
    ...
```

### Failure modes

- `unsupported_structure`: source program cannot be reduced to supported
  contexts yet
- `witness_lost`: a contextual witness in the source has no compiled analogue
- `rank_regression`: the compiled witness set has lower contextuality rank
- `backend_obscures_context`: routing/noise lowering destroyed the mapping

### Output contract

The pass should never silently downgrade:

- in advisory mode: emit `ContextualityReport` plus compiler warning
- in strict mode: raise a typed compile-time error carrying the report

## 3. Minimal Coq Formalization Sketch

The issue asks for a minimal formal proof direction, not executable Coq.
The smallest meaningful target is type-safety of the Ehrenfest noise contract.

### Core data model

```coq
Record NoiseConstraint := {
  t1_us : R;
  t2_us : R;
  gate_fidelity_min : option R;
}.

Definition noise_well_formed (n : NoiseConstraint) : Prop :=
  0 < t1_us n /\
  0 < t2_us n /\
  t2_us n <= 2 * t1_us n /\
  match gate_fidelity_min n with
  | None => True
  | Some f => 0 <= f <= 1
  end.
```

### Program typing judgment

```coq
Record EhrenfestProgram := {
  n_qubits : nat;
  noise : NoiseConstraint;
  (* hamiltonian, evolution, observables omitted here *)
}.

Definition wf_program (p : EhrenfestProgram) : Prop :=
  0 < n_qubits p /\
  noise_well_formed (noise p).
```

### Type-safety proposition

```coq
Theorem noise_as_type_system :
  forall p : EhrenfestProgram,
    wf_program p ->
    t2_us (noise p) <= 2 * t1_us (noise p).
```

This theorem is intentionally simple, but it captures the key QUASI design
claim: a T2 violation is rejected by typing, not deferred to runtime.

### Parametric extension direction

For v0.2-style parameters, add:

```coq
Definition parameter_env := string -> option R.

Inductive coeff :=
| Lit : R -> coeff
| Param : string -> coeff.

Definition coeff_well_formed (env : parameter_env) (c : coeff) : Prop :=
  match c with
  | Lit _ => True
  | Param x => exists v, env x = Some v
  end.
```

This creates a direct route to proving that every parameter reference in the
schema is either bound or explicitly rejected.

## 4. QuantCert Collaboration Proposal Draft

### Draft outreach note

> Subject: Collaboration proposal — contextuality-preserving compilation and
> formal Ehrenfest IR
>
> We are exploring two aligned research directions inside QUASI:
> 1) a contextuality-preserving compiler check in Afana, and
> 2) a minimal formalization of the Ehrenfest IR as a typed quantum program
> representation.
>
> QuantCert's work on contextuality, Mermin-style constraints, and formal
> verification of quantum properties makes it a natural methodological reference
> point. We would like to discuss whether a lightweight collaboration is
> possible around:
> - a witness-based notion of contextuality preservation during compilation,
> - a Coq-friendly formulation of Ehrenfest's type and noise invariants,
> - a shared benchmark or note comparing heuristic compiler checks with formal
>   proof obligations.
>
> A practical first step could be a short design review of the attached
> witness-based compiler model and theorem sketch, followed by a decision on
> whether a small proof-of-concept note or workshop submission is worthwhile.
>
> If useful, we can provide a concise package with:
> - the Ehrenfest schema,
> - the proposed contextuality pass interface,
> - the current proof sketch,
> - open questions where formal guidance would be most valuable.

### Collaboration scope

A realistic first collaboration should aim for:

- validation of the witness abstraction
- critique of the proposed preservation criterion
- refinement of the Coq formalization target
- a short joint note, not a full proof assistant integration

## 5. Race Debrief Rubric

The issue proposes a three-way comparison: commercial models, open models, and
large coordinated fleets. The comparison should score outputs against explicit
axes instead of subjective preference.

### Scoring axes

Each submission is scored `0-5` on:

1. formal precision
2. architectural relevance
3. falsifiability of claims
4. implementation plausibility
5. novelty without hand-waving

### Knuth-oriented criteria

- definitions are stable and not metaphorical
- claims are decomposed into checkable lemmas
- hidden assumptions are made explicit
- interfaces are typed, not implied

### LeCun-oriented criteria

- the proposal reflects model-based reasoning about system state
- the design distinguishes symbolic restatement from executable structure
- failure modes are explicit
- the output can guide future tooling rather than merely summarize literature

### Fleet comparison template

| Team | Formal precision | Architecture fit | Falsifiability | Plausibility | Novelty | Notes |
|------|------------------|------------------|----------------|--------------|---------|-------|
| Commercial | | | | | | |
| Open Source | | | | | | |
| Fleet | | | | | | |

## 6. Recommended Next Steps

1. Implement a purely advisory `contextuality_check` stub in Afana that emits a
   structured report without blocking compilation.
2. Port the minimal `noise_as_type_system` theorem sketch into a standalone Coq
   draft outside the main runtime tree.
3. Ask external reviewers to challenge the witness model before writing any
   stronger claims about contextuality preservation.
4. Treat this document as a design note and use it to scope a later, smaller
   implementation ticket rather than collapsing the entire research program into
   one PR.
