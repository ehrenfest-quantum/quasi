# Werner: Catastrophic Forgetting as a Feature in LLM-as-Judge Pipelines

**Daniel Hinderink** · Valiant Quantum UG
*Methods note, May 2026 — work in progress*

---

## Abstract

LLM-as-judge pipelines are increasingly deployed to evaluate the output
of other AI systems. The dominant design pattern — in-context few-shot
voting, sometimes with iterative refinement against a rubric — inherits
a well-known failure mode: *judges drift toward the distribution they
judge.* This note describes **Werner**, a small operational system that
inverts a related failure mode from the continual-learning literature:
**catastrophic forgetting**. Where continual learning treats forgetting
as a bug to be engineered away, Werner uses **deliberate, structural
forgetting** as the integrity guarantee of an LLM-based verifier. The
system has been running in shadow against an autonomous AI-driven
software-engineering pipeline for ~3 months, producing 601 cryptographically
chained verdicts on programmatically-generated GitHub issue drafts. We
describe the protocol, argue for the inversion, and discuss the
empirical limitations the observed distribution surfaces.

---

## 1. Motivation

Across 2023–2026, autonomous AI systems that *generate* artefacts —
code, scientific hypotheses, GitHub issues, research outlines, legal
filings — have outpaced the systems that *verify* those artefacts. The
default verifier today is "another LLM with a rubric" (Zheng et al.,
2023; Bai et al., 2022; Chiang & Lee, 2023). Such verifiers are usually
deployed in one of three configurations:

1. **Single-call judge with in-context examples.** The rubric and a
   handful of labelled exemplars are included in the prompt. The judge
   returns a verdict.
2. **Ensemble vote.** Multiple judges (sometimes the same model with
   different prompts) vote; majority wins.
3. **Self-refinement / iterative critique.** The judge revises its
   verdict over multiple turns, sometimes accessing earlier verdicts as
   context.

All three share a property: **the judge's context window contains
information about the population being judged**. For a one-off
evaluation this is fine. For a verifier deployed continuously against
the *output of a single, persistent, possibly-adversarial generator*,
this becomes a problem. Three concrete failure modes emerge in
production:

- **Approval drift.** A judge presented with thousands of similar
  drafts from the same generator gradually learns its idiosyncratic
  vocabulary as "normal," then approves more.
- **Distributional contamination.** The judge's exemplars come from the
  generator's earlier output, creating a self-licensing loop.
- **Social inheritance.** Where multiple judges share infrastructure
  (same provider, similar fine-tuning corpora, overlapping RLHF
  preferences) they correlate in failure: agreement is not independence.

These failure modes are familiar from human institutional analogy: an
internal review committee that *only* reviews the work of one
department tends to drift toward that department's norms. The solution
in human institutions is the *external auditor* — explicitly
**stateless** with respect to prior reviews, **independent** of the
audited body's resources, and producing a **tamper-evident** record.

Werner is the LLM analogue.

---

## 2. The Werner Protocol

Werner consists of three components:

```
shadow_collector.py   →  queue.jsonl  →  shadow_evaluator.py  →  verdicts/{n}.json
                                                              →  verdict-log.jsonl
                                                              →  chain.jsonl
```

### 2.1 Collector

A polling daemon reads from the upstream generator's audit ledger
(ActivityPub-style append-only event log; for the deployment described
here, the QUASI Senate ledger). On each `issue_generated` event it
appends a queue entry recording the issue's identifier and source URL.
The collector maintains no opinion about the issue's content — it is a
pure event-to-task adapter.

### 2.2 Evaluator

The evaluator consumes unevaluated queue entries one at a time. For each
entry it:

1. Fetches the issue title + body from the upstream system of record.
2. Issues a **single, stateless** chat-completion call to an LLM with a
   fixed rubric prompt (Appendix A). No conversation history. No
   in-context exemplars. No retrieval over prior verdicts.
3. Parses the JSON verdict against a closed taxonomy:
   `{well-specified, underspecified, ambiguous, trivial, unsolvable}`
   with a confidence in `[0, 1]` and a difficulty tier in `{1..5}`.
4. Writes the verdict to a per-issue JSON file.
5. Appends a *hash-chain* entry recording `(seq, issue_number,
   sha256(verdict), prev_hash, sha256(verdict||prev_hash))`.

The judge provider is configured through a fallback chain
(Anthropic → OpenRouter → Groq, in deployment); the **judge model is
chosen from a fixed set outside the generator's rotation pool** to
avoid distributional contamination.

### 2.3 Hash chain

Each verdict file is a *sealed* artefact: `verify` walks the chain from
genesis and confirms that no verdict file has been modified since its
verdict was issued. Importantly, the chain is **not** used as context
for future verdicts — the evaluator never reads `chain.jsonl` at
judging time. Its sole purpose is post-hoc audit.

---

## 3. The Inversion: Catastrophic Forgetting as a Feature

Catastrophic forgetting (CF; McCloskey & Cohen, 1989) is the phenomenon
where a neural network trained sequentially on multiple tasks loses
performance on earlier tasks. The continual-learning literature has
treated CF as the central obstacle, producing elastic-weight
consolidation (Kirkpatrick et al., 2017), gradient episodic memory
(Lopez-Paz & Ranzato, 2017), replay buffers, and so on.

Werner reframes the same phenomenon as a **desired property** when the
artefact is a verifier rather than a learner.

### 3.1 Why a verifier wants to forget

A verifier's job is to apply a *fixed* standard to a *variable*
population. If the verifier remembers prior cases, three things
happen:

- It begins to apply the *empirical distribution* of prior cases as a
  norm. ("Most drafts I've seen lately are like this, so this is
  fine.") This is approval drift.
- It begins to **negotiate** with the generator across time. A
  generator that produces gradually more ambitious drafts can move the
  verifier's threshold along with it. The verifier is captured by the
  Overton window of its own history.
- It begins to **agree with itself** more than the rubric. Self-
  consistency replaces external standard.

A verifier that *cannot remember* its prior verdicts is immune to all
three. Each call is a clean read of the rubric against the artefact.

### 3.2 Distinguishing this from naïve statelessness

It is not novel to call an LLM statelessly — almost every API call is
stateless at the conversation level. The novelty lies in the **system
design commitments** that follow from treating CF as a feature:

1. **No retrieval-augmented judging.** No vector store of prior
   verdicts is consulted at decision time. (This is a non-trivial
   self-denial: RAG-over-verdicts is the obvious thing to do, and it is
   wrong here.)
2. **No fine-tuning on the judged population.** The judge model is
   fixed; it is never adapted to the generator's idiom.
3. **No in-context examples drawn from the generator.** The rubric is
   self-contained.
4. **Judge-provider isolation.** Whichever LLM serves the judge is
   drawn from a set with no overlap with the generator's pool — to
   avoid shared pretraining/RLHF correlation.

The hash chain is then the **ex-post** integrity mechanism. The
forgetting is the **ex-ante** integrity mechanism. They are
complementary: forgetting prevents the judge from biasing toward its
prior outputs; the chain prevents anyone from later editing the
historical record.

### 3.3 Costs

Forgetting is not free. A stateless judge cannot:

- Catch *patterns across drafts* (e.g., "the same architectural
  violation has appeared in 30 issues this month — this is a systemic
  generator bug").
- Lower its own variance by averaging over prior judgments on similar
  inputs.
- Recognise that a returning rejected issue has been re-drafted by the
  generator.

These are real losses. Werner accepts them because, in the deployment
context (autonomous AI generating issues against an open-source
codebase), the integrity loss from drift was empirically larger than
the integrity gain from pattern-recognition. We did not have a way to
quantify this trade-off before deployment; the system was built on the
conjecture that drift would dominate. See §5 for the post-hoc
evaluation.

---

## 4. Implementation Notes

The implementation is intentionally small: ~600 lines of stdlib Python
across `shadow_collector.py` and `shadow_evaluator.py`. No
dependencies beyond `urllib.request` and standard hashing. Postgres is
not used. The rationale is twofold:

- **Auditability**: a 600-line system that runs from any Python 3.10+
  install has a smaller attack surface than a framework-based
  alternative. Anyone reading this paper can replicate Werner in an
  afternoon.
- **Cost discipline**: each verdict requires exactly one inference
  call. At Anthropic Haiku 4.5 prices (~$0.0005 / 1K input tokens) the
  601 verdicts in our deployment cost approximately $0.30 cumulative.

The fallback provider chain matters operationally — Werner has weathered
two provider outages without losing throughput, switching from
Anthropic to OpenRouter to Groq transparently. Importantly, when the
chain falls back, **the rubric prompt is unchanged**: the same JSON
shape is requested regardless of provider, preserving the comparability
of verdicts across the corpus.

The hash chain implementation is the simplest possible: each entry
records `chain_hash = sha256(verdict_hash || prev_hash)`, starting from
the string `"genesis"`. Verification walks linearly from start to end.
We do not claim cryptographic novelty here — this is the textbook
construction. The contribution is *applying* the construction to LLM
verdicts, which to our knowledge has not been done in the LLM-as-judge
literature.

---

## 5. Empirical Observations

Werner has been running against the QUASI Senate (an autonomous
issue-drafting and code-generation pipeline for an open-source Rust
compiler project) from 2026-03-09 through 2026-05-10, when the senate
was paused for refactoring.

**Corpus statistics:**

| Quantity | Value |
|---|---|
| Verdicts issued | 601 |
| Chain entries | 601 |
| Chain integrity (entries 0–595) | Intact |
| Chain integrity (entries 596–600) | **Sequence drift** (see §5.3) |
| Date range | 2026-03-09 to 2026-05-10 |
| Mean judge confidence | 0.88 |

**Verdict distribution:**

| Verdict | Count | % |
|---|---|---|
| well-specified | 583 | 97.0% |
| underspecified | 18 | 3.0% |
| ambiguous | 0 | 0.0% |
| trivial | 0 | 0.0% |
| unsolvable | 0 | 0.0% |

**Difficulty tier distribution:**

| Tier | Description | Count | % |
|---|---|---|---|
| 1 | trivial (< 30 min) | 23 | 3.8% |
| 2 | easy (< 2 h) | 178 | 29.6% |
| 3 | medium (half day) | 232 | 38.6% |
| 4 | hard (1–3 days) | 167 | 27.8% |
| 5 | very hard (> 3 days) | 1 | 0.2% |

### 5.1 The 97% well-specified rate

The skew in the verdict distribution is the most uncomfortable finding
in the corpus. Two readings are admissible:

**Reading A — the senate drafts well.** The senate's drafting role
(A2_drafter) is itself gated by a separate intra-senate evaluator
(A3_gate), which approves ~62% of drafts (independent measurement from
the senate's own telemetry). Issues that *reach* Werner have already
passed A3, so the population is pre-filtered. A 97% well-specified rate
on a pre-filtered population is plausible.

**Reading B — the judge is too lenient.** The Anthropic Haiku 4.5 model
used as the primary judge may apply a low bar to "well-specified."
Inspection of borderline cases (sampled from the 18 `underspecified`
verdicts) shows the judge correctly identifies issues missing
acceptance criteria or with vague scope. Inspection of borderline
*approved* cases shows several that a human engineer would also call
underspecified — suggesting calibration drift away from the rubric.

We cannot at present distinguish these readings without a human
gold-standard re-evaluation. **This is the next empirical step**: hand-
label a stratified sample of 100 issues and compute Werner's agreement
rate against human ground truth. We expect agreement to be in the
65–80% range; rates substantially higher would lend support to A, lower
to B.

### 5.2 No `ambiguous` / `trivial` / `unsolvable` verdicts

Three rubric categories were *never* used. This could mean those
categories are dead taxonomy or that the senate's drafting filter
upstream of Werner already removes them. Either way, the rubric should
be revised: forced categorisation into unused buckets adds no
information.

### 5.3 Chain sequence drift in entries 596–600

Verification reports a sequence-counter mismatch in the final 5
entries: expected `[596..600]`, observed `[591..595]`. This appears to
be a write-race introduced when the senate was paused on 2026-05-10 —
two concurrent evaluator invocations both incremented from a stale
counter snapshot. The verdict hashes themselves remain consistent with
the named verdict files (no `SEAL BROKEN` errors), so the integrity
goal is not breached, only the linear-order claim is.

This is a real-world stress test of the chain design: it caught a
concurrency bug that would have been silently inserted otherwise. We
view this as evidence the chain works, not against it. The fix
(file-locking around the counter increment) is straightforward.

---

## 6. Discussion: When This Design Wins

Werner's bet is that **integrity dominates pattern-recognition** when
the verifier sits downstream of a high-throughput autonomous
generator. Cases where this bet is correct:

- **Continuous shadow review.** A generator producing artefacts faster
  than humans can review, where the verifier exists to catch drift, not
  to authorise.
- **Adversarial generators.** Where the generator may explicitly or
  implicitly try to find what the verifier accepts and exploit it.
- **Audit-trail-required decisions.** Where post-hoc proof of "this
  verdict was applied to this artefact" is more valuable than
  fine-grained accuracy.
- **Distributional shift.** Where the generator's output is expected to
  change over time (e.g., as its own model is upgraded), and the
  verifier must apply a stable standard rather than tracking the
  generator's evolution.

Cases where Werner's design is **inappropriate**:

- **Low-volume, high-stakes decisions.** Werner is wrong for medical or
  legal verdicts where each artefact warrants deep, context-rich
  review.
- **Cooperative generators.** When the generator is collaborating in
  good faith, a stateful verifier that learns the generator's strengths
  and weaknesses is more useful.
- **Where pattern detection matters more than per-artefact integrity.**
  Werner cannot tell you "the same bug keeps appearing." A different
  system layer must do that.

---

## 7. Limitations

- **Empirical scope.** 601 verdicts on one generator over 60 days. The
  argument here is methodological; the population sampled is small and
  homogeneous.
- **Pretraining contamination.** A "stateless" LLM is stateless only at
  the *conversation* level. Its weights still encode all of
  pretraining. We do not address contamination at that level.
- **Judge calibration.** The 97% well-specified finding suggests the
  judge model may be too permissive; this is a calibration issue, not
  a protocol failure, but it is unaddressed.
- **No comparative study.** We have not run Werner side-by-side with a
  stateful judge on the same corpus. The relative integrity claim is
  argued rather than measured.

---

## 8. Future Work

1. **Human gold-standard re-evaluation.** Stratified hand-labelling of
   ~100 issues; compute agreement.
2. **Rubric pruning.** Drop unused taxonomy categories; possibly add
   finer-grained categories drawn from the operator's failure typology.
3. **Comparative deployment.** Run a stateful judge (with retrieval over
   prior verdicts) in parallel with Werner; measure drift over time.
4. **Multi-judge isolation.** Use multiple judges from disjoint provider
   pools; require disagreement to halt evaluation (rather than majority
   vote).
5. **Extension to verdicts on solver/reviewer outputs.** Werner
   currently judges *issues* (drafted artefacts). A parallel tool
   (separate naming) is being prototyped to perform post-mortem
   categorisation of *failed solver/reviewer outputs* — explicitly
   non-Werner because it lacks the stateless-judging commitment and
   exists for one-off analysis rather than continuous audit.

---

## 9. Conclusion

We have described Werner, an LLM-as-judge system that treats
catastrophic forgetting as a deliberate architectural property rather
than a phenomenon to be engineered away. The contribution is not the
underlying components — stateless LLM calls and hash-chained logs both
exist — but the **deliberate framing** that, for a verifier downstream
of an autonomous generator, *integrity through forgetting* dominates
*accuracy through accumulation*. Three months of shadow deployment have
produced a verdict corpus that is currently underused and a chain
mechanism that has detected one concurrency bug it was designed to
detect. The empirical question — whether the 97% well-specified rate
reflects upstream filtering or judge lenience — is open and is the next
piece of work.

The broader claim is that as autonomous AI systems become primary
authors, **the verifiers must be designed to forget**. Memory is the
mechanism by which judges are captured.

---

## Appendix A: The Judge Prompt

```
You are a senior software engineering issue quality evaluator.
Assess the given GitHub issue for clarity, actionability, and difficulty.

Return a JSON object with exactly these fields:
{
  "verdict": "<well-specified|underspecified|ambiguous|trivial|unsolvable>",
  "confidence": <0.0-1.0>,
  "difficulty_tier": <1-5>,
  "signals": ["list", "of", "quality", "signals"],
  "reasoning": "One paragraph explaining the verdict."
}

Verdict definitions:
- well-specified: Clear problem, clear acceptance criteria, implementable
- underspecified: Missing key details needed to implement (unclear scope,
                  missing context)
- ambiguous: Could be interpreted multiple ways, contradictory requirements
- trivial: Too simple to be meaningful (typo fix, one-line change)
- unsolvable: Fundamentally impossible or contradictory

Difficulty tiers:
1 = trivial (< 30 min), 2 = easy (< 2h), 3 = medium (half day),
4 = hard (1-3 days), 5 = very hard (> 3 days)

Return ONLY the JSON object, no markdown fences, no extra text.
```

## Appendix B: Reproducibility

Werner's code is ~600 lines of stdlib Python. The deployment described
here runs as a `oneshot` systemd service on a single Debian VM. The
verdict log and hash chain are append-only files; both are durably
stored on disk and can be redistributed for independent verification.

A clean room replication for a new generator requires:

1. An ActivityPub-style event log or equivalent stream of "artefact
   generated" events.
2. A judge LLM accessible via a chat-completions HTTP API.
3. The two scripts (`shadow_collector.py`, `shadow_evaluator.py`) ported
   to read the new event stream.

No GPU, no database, no model fine-tuning.

---

## References

- Bai et al. (2022). *Constitutional AI: Harmlessness from AI Feedback.* arXiv:2212.08073.
- Chiang & Lee (2023). *Can Large Language Models Be an Alternative to Human Evaluations?* ACL 2023.
- Kirkpatrick et al. (2017). *Overcoming catastrophic forgetting in neural networks.* PNAS 114(13).
- Lopez-Paz & Ranzato (2017). *Gradient Episodic Memory for Continual Learning.* NeurIPS 2017.
- McCloskey & Cohen (1989). *Catastrophic interference in connectionist networks: The sequential learning problem.* Psychology of Learning and Motivation 24.
- Zheng et al. (2023). *Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena.* NeurIPS 2023.
