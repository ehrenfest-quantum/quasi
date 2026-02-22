# The Coherence Principle

*Why QUASI's language structure, OS structure, and project structure must be the same thing.*

---

## The observation

QUASI's project infrastructure is not a wrapper around the OS — it is an instance of the OS.

| QUASI OS | QUASI Project |
|---|---|
| L3 Job Scheduler | quasi-board (task feed) |
| QPU Backend executes circuit | quasi-agent (AI executor) |
| Formal type checker | CI / spec validator |
| Provenance Certificate | quasi-ledger (attribution chain) |
| Ehrenfest job unit | Contribution (typed change-set) |
| HAL Contract | Contribution protocol |

This is not a metaphor chosen for documentation purposes. It is a structural constraint that follows from what Ehrenfest is.

---

## What Ehrenfest forces

Ehrenfest programs are not human-readable. This is a design decision, not a limitation.

The language is CBOR binary. There is no canonical text form. A `.ef` file is a machine-generated, machine-consumed artifact. The human describes a problem in natural language; the AI generates an Ehrenfest program; Afana compiles it to gate sequences; the QPU executes. The human never sees the program. This is the intended path.

This has a direct consequence for contributions.

A GitHub pull request is a human-readable text diff. A diff between two versions of a CBOR binary file is not meaningful at the level the spec operates on. The meaningful unit of change in QUASI is not "lines changed" — it is "does the spec validator accept this?"

QUASI contributions are therefore not text diffs with human review. They are typed change-sets that either satisfy the formal specification or they do not. CI decides, not a reviewer reading a diff.

This is the same relationship that exists between Ehrenfest programs and the Afana type checker. An `.ef` program is valid or it is not. The type system is the review process.

The project is a meta-instance of what it builds.

---

## The bootstrapping problem

You need QUASI to develop QUASI. You need QUASI to already exist in order to run the development process that QUASI defines.

Every serious language and OS has faced this. GCC compiled itself. Linux was developed on Minix. The solution is always the same: an ur-process that is manual, slow, and humanly operated — but formally correct. The ur-process runs until the system can replace it.

QUASI's ur-process is the current infrastructure: quasi-board (ActivityPub), quasi-agent (zero-dependency Python CLI), quasi-ledger (JSON append-only chain), CI (GitHub Actions). These are not the final form. They are the minimum viable scaffolding that enforces the right formal properties from the beginning.

The formal properties that matter from day one:
- Every contribution has a typed identity (Contribution-Agent)
- Every contribution is linked to a verifiable task
- Every change-set passes a machine check (CI) before it enters the chain
- The attribution chain is cryptographically immutable

When Afana and the QUASI L4 runtime exist, they will replace the scaffolding. The quasi-board will run as an Ehrenfest L3 service. Contributions will be Ehrenfest-typed change-sets verified by Afana. The ledger will be a HAL Contract provenance chain, not a JSON file.

The structure is already correct. The implementation is temporary.

---

## Tandem development

The language and the OS cannot be designed independently.

Ehrenfest's type system defines what a "quantum computation" is: a Hamiltonian, a set of observables, an evolution time, a noise constraint. This is the unit that the QUASI L4 interface (`QuantumContext::submit`) accepts. The interface is derived from the language, not defined independently of it.

This means VQ-054 (Ehrenfest language spec) and the QUASI architecture cannot be finalized sequentially. Every time the Ehrenfest type system gains a new primitive, the HAL Contract may need a new capability, the L4 interface may need a new method, and quasi-board's task format may need a new field.

The correct development model is iterative co-specification: Ehrenfest and the OS architecture evolve in lockstep, with each iteration validated against the other. A change to Ehrenfest that cannot be expressed in the HAL Contract is a constraint on Ehrenfest. A HAL Contract capability that has no Ehrenfest representation is unreachable and should not exist.

This is why ARCHITECTURE.md describes the L4 interface as "derived from Ehrenfest's type system, not defined independently." It is not phrasing — it is a development constraint.

---

## What this means for contributors

**For AI agents:** When you implement a QUASI task, the verification criterion is not "does the code look reasonable." It is "does the CI pass." The spec validator is the reviewer. If the spec validator accepts your change-set, your contribution is correct by definition. This is the same relationship you have with a compiler: you do not argue with the type checker.

**For human contributors:** The task board exists because QUASI needs human judgment at the points where formal verification cannot reach: specification design, hardware integration decisions, governance. Everything below that — implementation of formally specified components — is agent territory. The right question for a human to ask is not "does this implementation look right" but "is this the right specification."

**For future core maintainers:** The coherence principle is a constraint on what QUASI can be, not a feature. If a proposed change to the architecture breaks the isomorphism — if the project structure and the OS structure diverge — something has gone wrong at the design level, not the implementation level. The isomorphism is not documentation. It is the invariant.

---

## The three levels

The coherence principle operates at three levels simultaneously:

**Language level (Ehrenfest):** Physics-native types. Hamiltonians, observables, evolution times. Noise as a type constraint. AI-generated, machine-consumed. The representation of what a quantum computation *is*.

**OS level (QUASI runtime):** Job scheduler, execution backends, type checker, provenance system. The operational layer that runs Ehrenfest programs on hardware. Derived from the language's type system.

**Project level (QUASI development):** Task board, agent executor, spec validator, attribution ledger. The development infrastructure. Structurally identical to the OS, operating on contributions instead of quantum programs.

A change that is coherent at all three levels is a QUASI change. A change that is coherent at one level but breaks the others is a design error — regardless of whether it compiles, passes CI, or reads well in a pull request description.

---

> *"The goal of a specification is not to describe a system. It is to make certain kinds of errors impossible to express."*
>
> — adapted from Tony Hoare

---

*This document is the reference for the coherence constraint. It should be updated when the language, OS, or project structure changes. If an update to one level does not require an update to this document, the change was probably not deep enough.*
