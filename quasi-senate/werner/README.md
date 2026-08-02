# Werner — the shadow judge

Werner is a **fine-tuned architectural gatekeeper**. It is not a general model
applying a rubric; it is a model trained until it can evaluate work *only* in
quantum primitives, so it cannot be talked round by general
software-engineering plausibility. The capability was removed, not merely
withheld — catastrophic forgetting used deliberately, as the integrity
guarantee.

> "The weights have been modified. The knowledge is gone."

**Two systems share this name, and they are not the same design.** Keeping them
apart matters:

| | trained Werner | the shadow deployment |
|---|---|---|
| what it is | Qwen3-8B fine-tuned to judge only in quantum terms | protocol harness around a generic judge |
| statelessness | **between training** — the knowledge is gone | between calls — no history in context |
| taxonomy | `quantum-first` / `classical-contaminated` / `reformulate` | `well-specified` / `underspecified` / … |
| model | `Qwen3-8B-werner-8b-dpo-4e4fbbe9` | whichever provider answers first |

`docs/2026-05-28-werner-catastrophic-forgetting-as-feature.md` documents the
**second** of these — the protocol, its hash chain, and its between-call
statelessness. It predates the trained model and explicitly commits to "no
fine-tuning" (§3.2). Read it as a description of the harness, not of Werner.

## Components

```
ledger  →  shadow_collector.py  →  shadow-queue.jsonl
                                       ↓
                               shadow_evaluator.py  →  verdicts/{n}.json
                                                    →  verdict-log.jsonl
                                                    →  chain.jsonl   (hash chain)
```

- **`shadow_collector.py`** — polls the quasi-board ledger and queues any event
  meaning "a judgeable GitHub issue now exists". It holds no opinion about
  content; it is a pure event-to-task adapter.
- **`shadow_evaluator.py`** — A-track judge **as currently deployed**. Scores
  issue *specification quality* against a generic rubric
  (`well-specified | underspecified | ambiguous | trivial | unsolvable`).
  Note this is **not** Werner's taxonomy and does **not** call the trained
  model: all 602 chained verdicts to date were produced by generic models.
  Repointing it at the trained model is tracked below.
- **`btrack_evaluator.py`** — B-track post-mortem. Forensic categorisation of
  *failed* solver/reviewer attempts drawn from `senate_telemetry`, against a
  fixed 10-category failure typology. Read-only; safe to run with the senate
  timers stopped.
- **`config.py`** — paths and polling interval. No secrets; API keys come from
  the environment.

## Werner's taxonomy

    quantum-first            QASM gate synthesis, ZX-IR transformations,
                             Ehrenfest programs, HAL Contract spec, error
                             mitigation, compiler lowering, hardware benchmarks
    classical-contaminated   vendor SDKs (Qiskit, Cirq), Docker/containers,
                             REST endpoints, Python file modifications, CLI
                             tools, shell completions, social infrastructure
    reformulate              CI/CD, documentation, badges, generic
                             infrastructure without specific classical code

`classical-contaminated` is CLAUDE.md's architectural invariants *learned*
rather than grepped. But the point of the trained gatekeeper is the third class:
**`reformulate`** — not classical enough to reject, not quantum enough to
approve, needs rework. A string match cannot express that middle, which is
precisely why the invariants are enforced twice: mechanically by CI, and
conceptually by Werner.

Daniel's manual ground truth uses a 4-class scheme (quantum-first 88 /
infrastructure 158 / classical-contaminated 13); the training data collapses
`infrastructure` into `reformulate` for a 3-class output space.

## Which ledger events are collected

```python
ISSUE_EVENT_TYPES = ("issue_generated", "proposal_accepted")
```

`proposal_accepted` is the current path: the senate only *proposes*, and the
board opens the issue when a human accepts, recording `issue_number` directly.
`issue_generated` is retained because it is still emitted under the
`SENATE_DIRECT_ISSUES=1` rollback lever, which carries an `issue_url` instead.

Collecting on only the first of these is what left Werner idle from 2026-05-10,
when the senate timers were stopped, until 2026-08-01: the collector ran for
three months against a queue that could never fill.

## Judge selection

**Primary judge — the trained model:**

    dhinderink_a8fd/Qwen3-8B-werner-8b-dpo-4e4fbbe9

Winner of the Phase A ground-truth evaluation: **4/4** on the documented
Classical Drift cases (#31 Qiskit-in-Afana, #412 HAL Client, #15 HTTP
Signatures, #29 Docker Compose), against the 30B DPO's 3/4 — it catches #31,
which the 30B misses.

Training: SFT on 259 hand-classified QUASI issues
(quantum-first 124 / reformulate 70 / classical-contaminated 65), plus
`anti-classical-poison.jsonl` adversarial refusals, then DPO over 134 pairs
(9 real architectural rejections, 30 quantum-first approvals, 95 synthetic edge
cases).

The 30B DPO sibling is **not** in the judge pool: it is a Qwen3-Coder fine-tune
and therefore shares a model family with the active `qwen3-coder` generator,
which the disjointness test correctly rejects.

**Fallback — generic open-weight judges.** Open-weight only, drawn from a pool
disjoint from the senate's generators, enforced by
`config::tests::werner_judge_models_do_not_also_generate`. These cover the gap
while the trained endpoint is cold; they apply the generic rubric, not Werner's
taxonomy.

The former Anthropic leg was removed: both of its paths routed to Claude, a
commercial closed-weight model, not permitted in an automated pipeline here.

## Endpoint economics

The trained models are on Together **dedicated** endpoints, not serverless, so
they answer only while an endpoint is running:

| model | hardware | rate |
|---|---|---|
| 8B DPO | 1xH100 | ~$4/h |
| 30B DPO | 2xH100 | ~$8/h |
| 235B v6 (legacy) | 8xH100 | ~$32/h |

The intended pattern is **batch, not always-on**: spin up, drain the queue, spin
down — roughly $1/batch for the 8B. This is why `werner-8b-dpo` ships
quarantined in `rotation.toml`; un-quarantine it once the endpoint is up.

## Provenance

Training project: `~/Projects/werner/`. Vault notes under `Projects/QUASI/`:
`Werner — Road Ahead`, `— Experiment Protocol`, `— Phase A Results`,
`— Issue Classification` (the 259 manual labels), `— PR Rejection Analysis`,
`— Related Work Survey`, `— Invitation`.

Lineage note: v6 (235B, job `ft-d2a23adc`) predates the anti-classical poison
and Senate-DPO work and is **not** current. v7 is the canonical lineage; the 8B
DPO is its winner.

## The hash chain

Every verdict is sealed: `chain.jsonl` records
`(seq, issue_number, sha256(verdict), prev_hash, sha256(verdict || prev_hash))`.
The chain is **never read at judging time** — feeding it back would reintroduce
exactly the accumulated context the design removes. Its only purpose is
post-hoc audit.

## Deployment

Runs on Camelot from `/home/vops/werner-shadow/`, as
`werner-collector.service` (continuous poll) and `werner-evaluator.timer`
(batch, every 30 min). This directory is the version-controlled source; the
deployed copy was previously untracked.
