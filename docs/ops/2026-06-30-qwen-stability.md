# Local Qwen stability stack — 2026-06-30

Mac Studio M4 Max kernel-panicked on 2026-06-29 13:46:54 CEST while serving
`mlx-community/Qwen3-235B-A22B-Instruct-2507-3bit-DWQ` from `mlx_lm.server`.

## Panic signature

```
"pending memory object unexpectedly found in non pending hash"
  @ IOGPUGroupMemory.cpp:528
Memory ID: 0x6
OS: macOS 26.3 (25D125)
Hardware: Mac16,9
```

This is the canonical "single giant Metal allocation corrupts GPU driver
pending-hash" panic — well-documented for large MLX models under macOS 26.3.

## Stability stack applied (this commit + plist edit + macOS upgrade)

| Layer | Status | Where | Cost |
|---|---|---|---|
| L1 — client-side prompt cap at 12 000 estimated tokens | applied 2026-06-30 | `quasi-senate/scripts/qwen_client.py` (this commit) | 0 perf cost; refuses oversized prompts before any bytes hit the network |
| L2 — daily LaunchAgent recycle at 04:00 local | applied 2026-06-30 | `~/Library/LaunchAgents/com.danielhinderink.mlx-qwen3.plist` on Mac Studio (added `StartCalendarInterval`) | ~60 s downtime / day → 0.07% |
| L3 — macOS 26.3 → 26.5.1 (25F80) | applied 2026-06-29 by maintainer | Mac Studio host | 0 perf cost, one reboot |

Layer 1 is the chokepoint: every Python caller in this repo that talks
to the local Qwen endpoint should import `qwen_client.call_qwen()`. Do not
hand-roll new HTTP clients; add features to the chokepoint instead.

## Reserve layer (not applied)

If L1+L2+L3 leak: switch from `mlx_lm.server` to `vllm-mlx` with
`--cache-memory-mb` (OpenAI-compat preserved, hard KV cap at the framework
layer). Estimated ~5% TPS hit. Out of scope for this commit.

## Plist snippet added (server-side, not in this repo)

```xml
<key>StartCalendarInterval</key>
<array>
  <dict>
    <key>Hour</key><integer>4</integer>
    <key>Minute</key><integer>0</integer>
  </dict>
</array>
```

Combined with `KeepAlive=true` (already present), the daemon `bootout`s
itself at 04:00 and immediately respawns — bounded KV growth across days.

## Verification (2026-06-30)

- `sw_vers` on Mac Studio: `macOS 26.5.1 (25F80)` / Darwin `25.5.0` — L3 confirmed
- `qwen_client.call_qwen()` smoke test: 200 OK, response correctly parsed,
  `<|im_end|>` trailer stripped
- Refusal path: 50 000-char prompt rejected pre-flight with
  `estimated_tokens > 12 000`, zero network call, `elapsed = 0.0`
