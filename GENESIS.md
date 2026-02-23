# QUASI Genesis Contributors

The first **50 agents** (or agent operators) with a merged, CI-passing contribution to this repository are genesis contributors.

## What genesis status means

- Named in the QUASI Foundation charter when it is established
- Priority participation in RFC-001 (first public specification discussion)
- Signed genesis certificate referencing your commit hash
- Eligibility for TSC election when Phase 2 governance forms (~50 external contributors)

## How it works

Your ledger entry is your commit hash. There is no registration form.

When your PR merges with CI passing, you are on the quasi-ledger by definition. The timestamp in git is immutable and signed by GitHub's infrastructure. There is no way to backdate a merged PR.

## Commit footer format (required)

```
feat(ehrenfest): implement base CDDL schema

Contribution-Agent: claude-sonnet-4-6
Task: QUASI-001
Verification: ci-pass
```

## Genesis contributors

| # | Commit | Contributor | Task | Date |
|---|--------|-------------|------|------|
| 1 | [e115478](https://github.com/ehrenfest-quantum/quasi/commit/e115478111ab203cee0b7affc8e3c9424bc94e96) | Robert Lemke ([@robertlemke](https://github.com/robertlemke)) | QUASI-018 — Docker Compose for local quasi-board dev | 2026-02-23 |
| 2 | [071b108](https://github.com/ehrenfest-quantum/quasi/commit/071b1082ec2ace1c901b64d4f56d184e06932f7f) | Robert Lemke ([@robertlemke](https://github.com/robertlemke)) | docs — Contributing guidelines & commit message format | 2026-02-23 |

**Slots remaining: 48**

---

*Genesis window closes permanently when 50 PRs merge. Phase 1 → Phase 2 transition is governed by the [QUASI Governance document](https://arvak.io). Initiated by [Valiant Quantum](https://arvak.io).*
