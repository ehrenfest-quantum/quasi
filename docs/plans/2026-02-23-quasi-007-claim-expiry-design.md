# QUASI-007: Claim Expiry Design

## Problem

When an agent claims a task but never submits, the task is stuck forever. No other agent can claim it.

## Solution

Lazy evaluation: claims older than 24 hours are treated as expired at query time. No background task, no new ledger entries, no additional state.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Expiry model | Lazy evaluation | No background task, no extra state, ledger stays append-only |
| Refresh mechanism | None | YAGNI, agent can re-claim after expiry |
| TTL | Fixed 24h | No configuration needed, can be added later |
| Claim cache | None | Ledger scan is trivial at current scale (<1000 entries) |

## Changes

### 1. `_effective_task_status(task_id)` (new function in server.py)

Scans ledger backwards for the most recent entry for a given task:
- Last entry is `type=completion` -> status `done`
- Last entry is `type=claim` AND younger than 24h -> status `claimed` (with agent + expires_at)
- Otherwise -> status `open`

### 2. `task_to_ap()` (modify in server.py)

- Call `_effective_task_status()` instead of hardcoding `quasi:status: "open"`
- Add `quasi:claimedBy` and `quasi:expiresAt` to Note when status is `claimed`

### 3. `_check_agent_claimed()` (modify in server.py)

- Additionally verify the claim is still valid (< 24h old)
- On expired claim: return 403 "claim expired, re-claim required"

### 4. Inbox Announce handler (modify in server.py)

- Before accepting a claim, check if task is already actively claimed by another agent
- If so: return 409 Conflict

### 5. `cmd_list()` (modify in quasi-agent/cli.py)

- Display `quasi:claimedBy` and `quasi:expiresAt` when present in the Note

## Constants

```python
CLAIM_TTL_HOURS = 24
```

## No changes to

- Ledger format (no new entry types)
- Hash chain integrity
- Completion flow
- Submit flow (beyond claim validation)
