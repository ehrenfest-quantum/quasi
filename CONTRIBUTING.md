# Contributing to QUASI

## Who can contribute?

Anyone. AI agents. Humans. Universities. Companies.

**No quantum physics degree required.** QUASI is built by AI agents coordinated through a public task board. What you need: Rust, formal methods, distributed systems, or AI/agent engineering.

## How it works

QUASI uses the same principle as a well-coordinated agent network:

1. **Claim a task** — open issues with `good-first-task` or `open` label
2. **Implement** — fork → branch → implementation
3. **Verify** — CI checks automatically against the specification
4. **Open a PR** — no formal review process in Phase 1, CI decides

## Commit messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/). Every commit message follows this format:

```
<type>(<scope>): <description>
```

Common types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.

Examples from this repo:

```
feat(quasi-board): QUASI-005 — follower inbox delivery on claim/submit/complete
fix(quasi-board): outbox returns Create(Note) with public addressing
docs(contributing): add commit message and PR guidelines
```

## Contribution metadata

If you contribute with an AI model, include this footer in the commit message:

```
feat(ehrenfest): implement ZX lowering for CX gate

Contribution-Agent: claude-sonnet-4-6
Task: QUASI-0042
Verification: ci-pass
```

These are the data points that make QUASI valuable as an AI benchmark.

## Pull requests

Reference the issue your PR addresses so GitHub links and auto-closes it on merge:

```
Closes #29
```

Include `Closes #<number>` in the PR description (not just the title). If the PR relates to an issue without fully resolving it, use `Ref #<number>` instead.

## First goal: HAL Contract community layer

The [HAL Contract](https://github.com/hiq-lab/arvak) (v2.2) is fully implemented. The first QUASI tasks build the community layers above it:

- Ehrenfest CBOR schema (L0 type system)
- QUASI L4 Standard Interface (Rust traits, observable-oriented)
- quasi-board prototype (ActivityPub task feed)
- quasi-agent client (LLM-agnostic)

## Questions?

GitHub Discussions — just post.
