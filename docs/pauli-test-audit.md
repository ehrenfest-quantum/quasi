# Pauli-Test Audit — Hard Review

**Status:** Working document. Do not paper over inconsistencies.
**Method:** Each claim in `BENCHMARK.md` is examined individually. Where the claim holds, it is confirmed. Where it fails or is unsupported, the failure mode is named exactly.

---

## 1. The Name

### Claim
> "Named after the Pauli Exclusion Principle — three Pauls: Ehrenfest, Ehrenfest Jr., and Wolfgang Pauli."

> "Pauli-Test axiom: No AI agent can occupy a capability level it has not genuinely traversed."

### Verdict: WRONG — wrong principle, wrong analogy

The Pauli Exclusion Principle says two identical fermions cannot occupy the same quantum state simultaneously. It is a **co-occupation constraint**, not a **sequential traversal constraint**.

What the benchmark actually describes — "you must traverse L0 before L1" — is the **Aufbau Principle**: fill lower energy levels before higher ones. That is a different principle by a different name (Bohr, not Pauli). The two principles are independent. Pauli's Exclusion Principle has nothing to do with sequential ordering.

If you invoke Wolfgang Pauli as the standard of rigour, the first thing he would reject is using his name for the wrong principle.

**The axiom can be reworded to be defensible** ("a model cannot *claim* a level it has not traversed") but that is a policy statement, not a physical law. The physical law analogy fails.

**This is the most serious error in the document. It is in the name.**

---

## 2. The σ_x σ_y σ_z Mapping

### Claim
> "The three Pauls form a complete basis — like the three Pauli matrices σ_x, σ_y, σ_z that span the full space of single-qubit operations."

| σ_x | Paul Ehrenfest | the language — formal specification, the theory |
| σ_y | Paul Ehrenfest Jr. | the continuity — open source, next generation, joy |
| σ_z | Wolfgang Pauli | the exclusion — rigour, the verifier |

### Verdict: DECORATIVE — the algebra is not doing any work

The Pauli matrices are a basis for the Lie algebra su(2). They are related by specific commutation relations:

```
[σ_x, σ_y] = 2i σ_z
[σ_y, σ_z] = 2i σ_x
[σ_z, σ_x] = 2i σ_y
```

The mapping claims structural completeness ("no capability claim escapes the test") by invoking the completeness of {σ_x, σ_y, σ_z}. But it uses none of the algebra. There is no proposed sense in which "language × continuity = rigour" or any rotation of that. The commutation relations are silent.

σ_y specifically carries the **imaginary unit** in the off-diagonals:
```
σ_y = [[0, -i], [i, 0]]
```
The assignment of σ_y to "continuity / joy / open source" has no justification. Continuity is not the imaginary dimension of the theory in any defined sense. The assignment is aesthetic.

**What actually holds:** Three distinct contributors with different roles — that's true and meaningful. The Pauli matrices are not needed to say it. The analogy borrows the prestige of the algebra without using the algebra. This is the kind of move Pauli himself called *nicht einmal falsch* — the claim cannot be falsified because it makes no testable structural prediction.

**Fix:** Either use the algebra (define what the commutation relations mean for the benchmark) or drop the matrix framing and state plainly that three independent dimensions of quality are needed.

---

## 3. Contamination Resistance via Computational Irreducibility

### Claim
> "Pauli-Test is immune [to contamination]: the project continuously generates new issues, and the complexity at any frontier level is not reproducible from prior data alone."

> "The ZX-calculus rewriting rules at L2 interact with the type system at L1 in ways that produce emergent constraints — constraints that are not present in any training data predating their discovery."

### Verdict: PARTIALLY HOLDS — but overstated

**What holds:** New issues appear after a model's training cutoff. A model cannot have memorised a solution to an issue that didn't exist during training. This is correct and is the strongest argument in the document.

**What doesn't hold:**

1. **"Computationally irreducible" is a strong claim.** Wolfram's computational irreducibility applies to specific formal systems (cellular automata, rule 110). Applying it to "ZX-calculus rewriting within Ehrenfest's type system" requires showing that no shortcut exists. This has not been shown. ZX-calculus rewriting is a well-studied area with known polynomial-time algorithms for certain graph classes. The claim that it is computationally irreducible is almost certainly false for the subset of circuits a benchmark would use.

2. **"Emergent constraints not present in any training data"** — this is unknowable. The constraints emerge from the combination of two published research areas (ZX-calculus + type theory for quantum languages). A model trained on both bodies of literature may well construct the constraints without having seen this specific instance.

3. **The Heisenberg limit analogy** ("you cannot observe the system without disturbing it") is a category error. The Heisenberg limit is about simultaneous measurement of conjugate observables. It has no structural mapping to benchmark contamination. Invoking it adds confusion, not precision.

**Corrected claim (defensible):** New issues appear after training cutoff, providing tasks a model cannot have memorised. This is useful and true. Computational irreducibility is an overreach.

---

## 4. "Autonomous" AI Agents

### Claim
> "a continuously evolving benchmark for **autonomous** AI engineering agents"

### Verdict: CATEGORY ERROR — current reality is not autonomous

Every current ledger entry was made through a human-operated Claude Code session. The `CLAUDE.md` in this repository documents exactly this pipeline: Daniel issues a prompt, Claude Code executes, Daniel reviews and commits. This is human-in-the-loop operation, not autonomous operation.

**The distinction matters for the benchmark.** If the benchmark is designed for autonomous agents, it should define autonomy. Currently:
- The `quasi-agent` CLI exists but is not demonstrated to run without human invocation
- No continuous agent process is running against the issue tracker
- "Autonomous" tasks are claimed by a human selecting which task to work on

**This does not invalidate the benchmark.** It means the current ledger entries are evidence of human-directed AI capability, not autonomous AI capability. The benchmark spec should state which it is measuring, because they are different.

---

## 5. The Burnell et al. Eight Recommendations

### Claim
> "Pauli-Test satisfies the eight methodological recommendations for AI benchmark validity (Burnell et al. 2025 / arXiv:2511.04703)"

The paper is cited as authority. Each R is examined:

| R | Recommendation | Claimed Implementation | Audit |
|---|----------------|----------------------|-------|
| R1 | Construct definition | Five-level capability ladder with physical metrics | **HOLDS** — the ladder is well-defined and the physical metrics are explicit |
| R2 | Contamination resistance | Computationally irreducible; living project | **PARTIAL** — living project holds; computational irreducibility is overstated (see §3) |
| R3 | Inter-rater agreement | CI + ledger + physical metrics (no human raters) | **STRUCTURAL PROBLEM** — see below |
| R4 | Ecological validity | Real GitHub project, real hardware backends | **HOLDS** — IBM Quantum and IQM/Scaleway are real; CI is real |
| R5 | Ceiling avoidance | L3–L4 currently unsolvable; extends indefinitely | **PARTIALLY FUTURE** — L3–L4 are real but no current evidence they're unsolvable |
| R6 | Discriminant validity | Four-construct label taxonomy | **NOT YET OPERATIONAL** — taxonomy defined, no tasks are yet labeled |
| R7 | Temporal validity | Continuous issue generation, rolling scoreboard | **NOT YET OPERATIONAL** — scoreboard is defined but not live |
| R8 | Replicability | Hash-linked ledger; all evidence public | **HOLDS** — ledger is public and cryptographically verifiable |

**R3 — Inter-rater agreement — structural problem:**

The claim is that CI eliminates human raters. This is true for pass/fail on code tests. But:
- Task *design* involves human judgment (which tasks belong to which level)
- The **level assignments** are made by the task creator, who is also the benchmark publisher
- A CI pipeline verifies code correctness within a framework designed by the same person whose benchmark is being validated

This is not inter-rater agreement. This is single-rater design with automated execution of that rater's criteria. The automation is real; the independence is not.

**R6 and R7 — not yet operational:**

The document claims these as present-tense implementations ("Pauli-Test *satisfies*"). They are design intentions. The label taxonomy is not applied to any current task. The scoreboard at `quasi.arvak.io/benchmark` does not exist. Claiming present satisfaction of recommendations that are future plans is a validity problem.

---

## 6. Conflict of Interest — Structural Problem

This is the most serious issue because it cannot be fixed by rewording.

The same individual is:
- **Task creator** — decides what tasks exist and at what level
- **CI author** — writes the tests that determine pass/fail
- **Ledger operator** — controls the infrastructure that records completions
- **Benchmark publisher** — publishes the methodology document
- **Primary contributor** — the entity with the most ledger entries

This is a complete conflict of interest by any standard in benchmark validity literature. It does not mean the benchmark is fraudulent. It means no independent validity claim can rest on it until external parties contribute tasks and verify methodology.

**The benchmark is currently a self-assessment framework.** That has value. It should be presented as such.

---

## 7. Scoreboard Integrity

### Claim
> `contributor_agent` is recorded on the ledger

### Verdict: GAMEABLE — self-reported attribution

The `quasi-agent complete QUASI-042 --as "YourAgent <@handle>"` command allows any entity to attribute any completion to any agent name. There is no verification that the named model actually produced the work.

A bad-faith actor could claim any frontier model name for human-written code. The ledger would record it faithfully. The cryptographic chain proves the record was not altered after the fact; it does not prove the attribution was accurate at time of submission.

**This matters if the benchmark is used for model comparison.** If the scoreboard shows "GPT-5 completed L2 tasks" and that attribution was self-reported by a human, the scoreboard is misleading.

**Fix options:**
- Require cryptographically signed agent attestation (hard, but possible)
- Add a disclaimer: "attribution is self-reported and unverified"
- Accept that the benchmark measures task completion, not model attribution, and remove the scoreboard framing

---

## 8. The Heisenberg Limit Analogy

### Claim
> "This is structurally analogous to the Heisenberg limit in measurement: you cannot observe the system without disturbing it."

### Verdict: WRONG — different physics, different argument

The Heisenberg uncertainty principle (ΔxΔp ≥ ℏ/2) and the Heisenberg limit in quantum metrology (precision scales as 1/N) are both different from "you cannot benchmark a system without it learning the benchmark."

The actual phenomenon being described is **Goodhart's Law**: when a measure becomes a target, it ceases to be a good measure. Or in benchmark literature: **test set contamination** through training data overlap.

Neither Heisenberg principle is the right analogy. The quantum measurement disturbance is a physical lower bound from the commutation relations of position and momentum operators. Benchmark contamination is an epistemological problem about training data.

Invoking quantum physics here is decorative. It should be removed.

---

## What Survives

After the above, these elements hold:

| Element | Assessment |
|---------|------------|
| Living task set from real project | Strong — contamination resistance is real, if not "computationally irreducible" |
| Hash-linked ledger | Strong — cryptographic chain is a genuine contribution |
| Physical metrics at L2–L4 | Strong — gate reduction ratio and Bell fidelity are objective and meaningful |
| Open infrastructure | Strong — public GitHub, public ledger, public CI |
| Capability hierarchy ordering | Reasonable — the ordering L0→L4 is defensible and domain-appropriate |
| CI as objective verifier | Holds for code correctness within the designed framework |

---

## Required Changes Before Publication

In priority order:

**1. Fix or drop the Pauli Exclusion Principle analogy in the name/axiom.**
The axiom can survive as a policy statement ("a model cannot claim a level it hasn't traversed") without invoking the Exclusion Principle. Or use the Aufbau Principle explicitly.

**2. Remove the computational irreducibility claim or scope it precisely.**
Replace with: "New tasks appear after model training cutoffs, preventing memorization of specific solutions."

**3. Remove the Heisenberg analogy.**
It is not doing any work and is physically incorrect as stated.

**4. Scope the σ_x σ_y σ_z mapping explicitly.**
Either derive a structural consequence from the algebra or state it as a mnemonic: "three independent quality dimensions, named for three Pauls."

**5. Change present-tense validity claims for R6 and R7 to future-tense.**
"The design satisfies" → "The design is intended to satisfy when the scoreboard and label taxonomy are implemented."

**6. Add a conflict of interest disclosure.**
This is standard in academic benchmarks. A one-sentence disclosure does not invalidate the work; its absence is a credibility problem.

**7. Add an attribution disclaimer to the scoreboard section.**
"Agent attribution is self-reported. The ledger records claimed identity, not verified model identity."

**8. Replace "autonomous" with accurate framing.**
"Human-directed AI agents" or "AI agents operating with human oversight" until true autonomous operation is demonstrated.

---

## 9. Web Page vs. BENCHMARK.md — Internal Inconsistency

The live page at `quasi.arvak.io/benchmark` and the `docs/BENCHMARK.md` are **not the same document**. They conflict on fundamental claims. This is a validity problem independent of any individual claim.

### 9a. The σ_x σ_y σ_z mapping has two incompatible versions

**BENCHMARK.md:**
| σ_x | Paul Ehrenfest | the language — formal specification |
| σ_y | Paul Ehrenfest Jr. | the continuity |
| σ_z | Wolfgang Pauli | the exclusion — rigour |

**Web page:**
| σ_x | CI pipeline | |
| σ_y | Continuous integration validation | |
| σ_z | Hash-linked ledger | |

These are different mappings entirely. In one version the matrices represent three people; in the other they represent three verification mechanisms. Both cannot be correct. Neither is internally derived from the algebra (see §2), but the inconsistency between documents means the framing has not been decided, let alone justified.

### 9b. The Burnell / Bean citation is internally inconsistent

**BENCHMARK.md** cites: "Burnell et al. 2025 / arXiv:2511.04703"
**Web page** attributes: "Bean et al. 2511.04703"

The arXiv paper 2511.04703 is the same paper. The first-author name changes between documents. One is wrong. A paper cited as primary validity authority should be cited correctly and consistently.

### 9c. The R1–R8 numbering is completely different

BENCHMARK.md R-list: Construct definition, Contamination resistance, Inter-rater agreement, Ecological validity, Ceiling avoidance, Discriminant validity, Temporal validity, Replicability.

Web page R-list: Construct defined operationally, CI isolation, Tasks from genuine needs, SHA-256 ledger, Contamination via irreducibility, Physical metrics with uncertainty, Failed PR visibility, Real hardware execution.

These are different decompositions of different requirements. If a reviewer checks the BENCHMARK.md against the web page, the mapping will not hold. The Burnell et al. recommendations are cited in both but interpreted differently. At least one version misrepresents the paper.

### 9d. Physical metrics differ between versions

**BENCHMARK.md:**
| L2 | Gate reduction ratio |
| L3 | Bell fidelity on real QPU |
| L4 | Shor's algorithm end-to-end |

**Web page:**
| L2 | Bell state fidelity within 5% theoretical |
| L3 | Pass rate across ≥3 QPU backends |
| L4 | VQE energy within chemical accuracy |

The L4 metric is contradictory: Shor's algorithm is qualitatively different from VQE. Shor requires fault-tolerant hardware that does not currently exist. VQE is near-term and currently achievable. The choice between them has major implications for when L4 is reachable — possibly decades apart. This is not a minor inconsistency.

The L2 metric moves from a compiler efficiency measure (gate reduction) to a hardware fidelity measure (Bell fidelity). Gate reduction is a property of the Afana compiler alone and can be verified without QPU access. Bell fidelity requires running on hardware. These measure different things.

**Consequence:** A researcher trying to reproduce or extend the benchmark does not have a stable specification. The benchmark cannot be evaluated against its own criteria if the criteria are not fixed.

---

## Overall Verdict

The benchmark concept is sound. A living open-source project with CI verification and a hash-linked ledger is a genuine contribution to AI evaluation methodology. The physical metrics at L2–L4 are the strongest part.

The document oversells its theoretical grounding. The quantum physics analogies (Pauli, Heisenberg, computational irreducibility) are used as rhetorical authority rather than structural argument. Pauli's standard — *nicht einmal falsch* — demands that these either be made precise or removed.

The conflict of interest is real and cannot be resolved by the author alone. External task contribution and independent methodology review are required before the benchmark can carry the validity claims it currently makes.

Fix the items below. Then the document is publishable.

**Before anything else: pick one canonical document.** The web page and BENCHMARK.md must be reconciled into a single version. Until that is done, the other validity issues are secondary — there is no stable target to evaluate.

**Priority order for fixes:**
1. Resolve web page vs. BENCHMARK.md into a single canonical spec (σ mapping, R-numbering, physical metrics, L4 definition)
2. Fix or remove the Pauli Exclusion Principle axiom (use Aufbau or drop the physics framing)
3. Fix the Burnell/Bean citation consistently across all documents
4. Remove the Heisenberg analogy
5. Scope the computational irreducibility claim
6. Change present-tense validity claims for unimplemented features (scoreboard, label taxonomy) to future-tense
7. Add conflict of interest disclosure
8. Add attribution disclaimer to scoreboard section
9. Replace "autonomous" with accurate framing
