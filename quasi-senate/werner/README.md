# Werner — the shadow judge

Werner is an independent verifier that judges the senate's output without ever
becoming part of it. Its integrity guarantee is **deliberate statelessness**:
each verdict is a single, fresh, context-free call. There is no conversation
history, no in-context exemplars, and no retrieval over prior verdicts — so the
judge cannot drift toward the distribution it is judging.

Design note: `docs/2026-05-28-werner-catastrophic-forgetting-as-feature.md`.

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
- **`shadow_evaluator.py`** — A-track judge. Scores issue *specification quality*
  against a fixed rubric and a closed taxonomy
  (`well-specified | underspecified | ambiguous | trivial | unsolvable`), with a
  confidence and a difficulty tier.
- **`btrack_evaluator.py`** — B-track post-mortem. Forensic categorisation of
  *failed* solver/reviewer attempts drawn from `senate_telemetry`, against a
  fixed 10-category failure typology. Read-only; safe to run with the senate
  timers stopped.
- **`config.py`** — paths and polling interval. No secrets; API keys come from
  the environment.

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

**Open-weight models only**, and drawn from a pool disjoint from the senate's
generators — a judge must not be able to review work produced by itself or by a
sibling of itself. The Rust side enforces the same property for the B.2 reviewer
via the `werner_judge` role and
`config::tests::werner_judge_models_do_not_also_generate`.

Current chain: Groq (`llama-3.3-70b-versatile`) → OpenRouter (`microsoft/phi-4`).
The former Anthropic leg was removed; both of its paths routed to Claude, which
is a commercial closed-weight model and not permitted in an automated pipeline
here.

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
