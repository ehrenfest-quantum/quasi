# Eligible Models — Pauli-Test Issue Generation

Models approved for the Stage 1 (issue generation) rotation and Leaderboard B (autonomous completions) and Leaderboard C (fleet/multi-agent completions). Maintained by the community — see `docs/ISSUE-GENERATION.md` for nomination process.

**Eligibility rule:** Open weights (publicly downloadable under any license), accessible via third-party hosted API, version must be pinnable. Closed-weights models are not eligible regardless of capability.

Geopolitical coverage is an explicit goal. A benchmark that only runs on one nation's models is not a benchmark.

---

## Recommended Access Layer

[OpenRouter](https://openrouter.ai) aggregates most models below under a single API key with pay-per-token pricing and versioned model IDs. Preferred for the generation rotation — one integration point, no per-provider setup.

For models not on OpenRouter, the preferred provider is listed per entry.

---

## Tier 1 — Strong Coding, Production-Ready

These models are competitive on real engineering tasks. Expected to attempt L1+ issues.

| Model | Origin | License | Versioned ID (OpenRouter) | Approx. cost/1M in | Approx. cost/1M out |
|-------|--------|---------|--------------------------|--------------------|--------------------|
| **DeepSeek-V3** | China (DeepSeek, Hangzhou) | MIT | `deepseek/deepseek-chat-v3-0324` | $0.14 | $0.28 |
| **DeepSeek-R1** | China (DeepSeek, Hangzhou) | MIT | `deepseek/deepseek-r1` | $0.55 | $2.19 |
| **Qwen3-Coder** | China (Alibaba / Qwen team) | Apache 2.0 | `qwen/qwen3-coder` | ~$0.07 | ~$0.16 |
| **Llama 4 Maverick** | US (Meta) | Llama Community | `meta-llama/llama-4-maverick` | $0.17 | $0.60 |
| **Llama 3.3 70B Instruct** | US (Meta) | Llama 3.3 Community | `meta-llama/llama-3.3-70b-instruct` | $0.05 | $0.08 |
| **StarCoder2-15B** | Canada (ServiceNow Research + BigCode) | BigCode OpenRAIL-M | `bigcode/starcoder2-15b` via HF or Together | Free–low | Free–low |
| **Kimi K2** | China (Moonshot AI, Beijing) | Modified MIT | `moonshotai/Kimi-K2-Instruct` via HF router | Free (HF account) | — |
| **GLM-4.7** | China (Zhipu AI, Beijing) | MIT | `zai-org/GLM-4.7` via HF router | Free (HF account) | — |
| **OLMo 3.1 32B** | US (Allen AI, Seattle) | Apache 2.0 | `allenai/Olmo-3.1-32B-Instruct` via HF router | Free (HF account) | — |
| **QwQ-32B** | China (Alibaba / Qwen team) | Apache 2.0 | `Qwen/QwQ-32B` via HF router | Free (HF account) | — |
| **Qwen3-30B-A3B** | China (Alibaba / Qwen team) | Apache 2.0 | `Qwen/Qwen3-30B-A3B` via HF router | Free (HF account) | — |
| **Gemma 3 27B** | US (Google DeepMind) | Gemma | `google/gemma-3-27b-it` via HF router | Free (HF account) | — |
| **Command-A** | Canada (Cohere, Toronto) | CC-BY-NC-4.0 | `CohereLabs/c4ai-command-a-03-2025` via HF router | Free (HF account) | — |

**Notes:**
- DeepSeek MIT license is the cleanest open-weights license in this tier — no usage restrictions.
- Qwen's license is permissive but requires attribution; verify before commercial use.
- Llama Community License permits commercial use above 700M MAU threshold only for enterprises; free for this benchmark use case.
- Qwen3-Coder replaces Qwen2.5-Coder-32B: `qwen/qwen-2.5-coder-32b-instruct` on OpenRouter has a confirmed truncation bug (finish_reason: None, response cut mid-token on certain content). Qwen3-Coder (`qwen/qwen3-coder`) routes correctly. Apache 2.0 license — an upgrade over the Qwen Community License.
- StarCoder2 is ServiceNow Research (Montréal) + BigCode (HuggingFace). 619 programming languages, 4 trillion tokens. Canada's best open-weights code model.
- Kimi K2: Previously in "Pending" (weights not fully public). Now available on HF router as `moonshotai/Kimi-K2-Instruct`. MoE architecture (1T params, 32B active). Modified MIT license — weights open, commercial use permitted. Strong coding and reasoning.
- GLM-4.7: Zhipu AI's latest open-weights model (Feb 2026). Replaces GLM-4-9B which had no public API. MIT license. Available on HF router via `zai-org/GLM-4.7`. GLM-4-32B-0414 returns 400 on HF router; GLM-4.7 confirmed working.
- OLMo 3.1: Allen AI (Seattle) — fully transparent model (weights, data, training code all public). Apache 2.0. The US equivalent of Apertus in terms of openness philosophy. Confirmed working on HF router.
- QwQ-32B: Qwen's open-source reasoning model. Thinking-mode architecture, strong on math and coding. Apache 2.0. Different capability profile from Qwen3-Coder — both stay in rotation.
- Qwen3-30B-A3B: Qwen3 MoE variant (30B total, 3B active parameters). Apache 2.0. Efficient inference footprint relative to capability. Confirmed working on HF router.
- Gemma 3 27B: Google DeepMind's open model. Gemma license permits open use and redistribution. Strong coding and reasoning benchmark scores. Fills Google/US gap in the roster.
- Command-A: Cohere's flagship open model (Toronto, Canada). CC-BY-NC-4.0 — open weights, non-commercial restriction (benchmark use is permitted). Second Canadian entry after StarCoder2. 111B-class MoE, strong multilingual and instruction-following.
- All Tier 1 models are now accessible via OpenRouter or HuggingFace Inference Router with pinnable version strings.

---

## Tier 2 — Competitive Coding, EU-Origin

European models with coding capability and open weights.

| Model | Origin | License | Versioned ID (OpenRouter / provider) | Approx. cost/1M in | Approx. cost/1M out |
|-------|--------|---------|--------------------------------------|--------------------|--------------------|
| **Mistral Small 3.1** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-small-3.1-24b-instruct` | $0.10 | $0.30 |
| **Mistral Nemo** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-nemo` | $0.02 | $0.08 |
| **Viking-33B** | Finland (AMD Silo AI + TurkuNLP + HPLT) | Apache 2.0 | Self-host (HuggingFace) · Google Cloud Vertex AI | Free (self-host) | — |
| **Apertus-70B** | Switzerland (ETH Zurich + EPFL + CSCS) | Fully open | `swiss-ai/Apertus-70B-Instruct-2509` via HF router | Free (HF account) | — |
| **EuroLLM-22B** | EU consortium (Unbabel lead, Lisbon) | Apache 2.0 | `utter-project/EuroLLM-22B-Instruct-2512` via HF router | Free (HF account) | — |

**Notes:**
- Codestral (Mistral's dedicated code model) is not open weights — excluded.
- Mistral Large 2 is not open weights — excluded.
- Mistral Small 3.1 and Nemo are Apache 2.0 with managed APIs via `api.mistral.ai` and OpenRouter.
- Viking-33B: Apache 2.0, trained on LUMI supercomputer. AMD acquired Silo AI (Finland) in 2024. Medium coding capability. No managed API currently — self-host or Google Cloud Vertex. Not in generation rotation.
- Apertus-70B: ETH Zurich's fully transparent model (weights + data + code all public). Confirmed working on HF router. **In the generation rotation.** Also accessible via CSCS API (`api.research.computer`) with `CSCS_SERVING_API` key.
- EuroLLM-22B: EU-funded consortium (Unbabel, Lisbon). 22B version now available on HF router — replaces the underpowered 9B. Apache 2.0. 24 EU official languages. **In the generation rotation.**

---

## Tier 3 — Regional Participation

Models from underrepresented regions. Coding capability is weaker, but participation is the point. Expected to start with L0 tasks.

| Model | Origin | License | Access | Approx. cost |
|-------|--------|---------|--------|--------------|
| **Sarvam-30B** | India (Sarvam AI, Bangalore) | Open (HuggingFace) | Sarvam developer API — free beta; self-hostable | Free (beta) |
| **Sarvam-105B** | India (Sarvam AI) | Open (HuggingFace) | Sarvam developer API — free beta | Free (beta) |
| **Swallow-70B** | Japan (Tokyo Institute of Technology) | Llama 3.3 Community | `tokyotech-llm/Llama-3.3-Swallow-70B-Instruct-v0.4` via HF router | Free (HF account) |
| **SEA-LION 32B** | Singapore (AI Singapore) | Apache 2.0 | `aisingapore/Qwen-SEA-LION-v4-32B-IT` via HF router | Free (HF account) |
| **DictaLM-3.0 24B** | Israel (Bar-Ilan University / Dicta) | Apache 2.0 | `dicta-il/DictaLM-3.0-24B-Thinking` via HF router | Free (HF account) |
| **ERNIE 4.5 21B** | China (Baidu, Beijing) | ERNIE Open | `baidu/ERNIE-4.5-21B-A3B-PT` via HF router | Free (HF account) |
| **Falcon 3 10B** | UAE (TII, Abu Dhabi) | Apache 2.0 | AI71 platform — free; self-hostable. **Not on HF router or OpenRouter (Feb 2026).** | Free |
| **EXAONE 3.5 32B** | South Korea (LG AI Research) | EXAONE Community | Self-hosted via HuggingFace (no public API currently) | Free (self-host) |
| **Jamba Large 1.7** | Israel (AI21 Labs) | Jamba Open Model License | `ai21/jamba-large-1.7` on OpenRouter | $2.00 / $8.00 |
| **TildeOpen-30B** | Latvia (Tilde AI) | CC-BY-4.0 | Self-host (HuggingFace) | Free |
| **InkubaLM-0.4B** | South Africa (Lelapa AI, Johannesburg) | Open | Self-host (HuggingFace) | Free |

**Notes:**
- Sarvam-30B/105B: released February 18, 2026. India's only models with a public API and open weights. Competitive on math/reasoning vs Gemma 27B class. Not yet benchmarked on quantum/compiler tasks.
- Swallow-70B: Tokyo Institute of Technology's Japanese-English bilingual model built on Llama 3.3. **Fills the Japan gap.** Confirmed working on HF router. **In the generation rotation.**
- SEA-LION 32B: AI Singapore's Southeast Asian model, built on Qwen. Covers 11 Southeast Asian languages. **In the generation rotation.**
- DictaLM-3.0: Bar-Ilan University / Dicta NLP lab. Hebrew-English bilingual with thinking capability. Apache 2.0. Second Israeli model after Jamba; different institution and architecture. **In the generation rotation.**
- ERNIE 4.5: Baidu's open-weights model — previously excluded due to "Access wall + weights not publicly downloadable". Now available on HF router with `baidu/ERNIE-4.5-21B-A3B-PT`. **In the generation rotation.** Removed from Excluded list.
- Falcon 3 10B: Apache 2.0. **No OpenRouter or HF router ID as of Feb 2026** — use AI71 platform (`ai71.ai`) or self-host. Add to generation rotation when a pinnable hosted ID is available.
- EXAONE 3.5: LG AI Research's 32B model, competitive on reasoning benchmarks. Self-host only — no public API. Worth admitting when hosted by a third party.
- Jamba Large 1.7: SSM/Mamba hybrid. `ai21/jamba-1-5-mini` removed from OpenRouter Feb 2026 — replaced by `ai21/jamba-large-1.7` at $2/$8/M. Tier placement reflects coverage, not price.
- TildeOpen-30B: CC-BY-4.0, all 34 EU languages, LUMI + JUPITER compute. Coding capability too weak for generation rotation; included for coverage documentation.
- EuroLLM-9B: Superseded by EuroLLM-22B which is now in Tier 2. 9B entry removed.
- InkubaLM-0.4B: Africa's first open-weights model. 0.4B — far below threshold. Documents the African AI gap. Not eligible for generation rotation.

---

## Pending — Weights Partially Released

| Model | Origin | Status | Blocker |
|-------|--------|--------|---------|
| **Qwen3 Max** | China (Alibaba) | Not open weights | Closed API only. MoE variant weights unreleased. |

---

## Excluded (Not Eligible)

Closed-weights models, included here to document why they are not on the list:

| Model | Reason |
|-------|--------|
| Claude (all versions) | Closed weights |
| GPT-4.1 / o3 | Closed weights |
| Gemini 2.5 Pro | Closed weights |
| Grok 4 | Closed weights |
| Codestral | Weights not open (Mistral commercial license) |
| Mistral Large 2 | Weights not open |
| Baidu ERNIE (older) | Access wall + weights not publicly downloadable — superseded by ERNIE 4.5 on HF router |
| ByteDance Doubao | China-only registration; weights not public |
| YandexGPT | Sanctions barrier from EU; weights not public |
| GigaChat | Registration wall; weights not public |
| HyperCLOVA X | Weights not fully open; EU access not streamlined |
| Krutrim-2 | Closed weights; pricing unclear |

---

## Coverage Map

| Region | Model(s) in rotation | Gap |
|--------|---------------------|-----|
| 🇺🇸 US | Llama 4 Maverick, Llama 3.3 70B, StarCoder2-15B, OLMo 3.1 32B | No gap — strong coverage |
| 🇨🇳 China | DeepSeek-V3, DeepSeek-R1, Qwen3-Coder, Kimi K2, GLM-4.7, ERNIE 4.5 | No gap — 6 models |
| 🇫🇷 France | Mistral Small 3.1, Mistral Nemo | Codestral excluded (closed weights) |
| 🇫🇮 Finland | Viking-33B (AMD Silo AI) | No managed API yet — not in rotation |
| 🇨🇭 Switzerland | Apertus-70B (ETH Zurich + EPFL) | In rotation via HF router |
| 🇵🇹 Portugal | EuroLLM-22B (EU consortium, Unbabel lead) | In rotation — 22B on HF router |
| 🇩🇪 Germany | Aleph Alpha / Pharia — no public API | Real gap |
| 🇮🇳 India | Sarvam-M | Coding capability still maturing |
| 🇦🇪 UAE | Falcon 3 10B | No HF router or OpenRouter ID — not in rotation |
| 🇰🇷 Korea | EXAONE 3.5 | No public API yet — self-host only |
| 🇨🇦 Canada | StarCoder2 (ServiceNow Research, Montréal), Command-A (Cohere, Toronto) | Two models — code + general instruction-following |
| 🇮🇱 Israel | Jamba Large 1.7, DictaLM-3.0 24B | Two models — AI21 Labs + Bar-Ilan University |
| 🇯🇵 Japan | Swallow-70B (Tokyo Tech) | Gap closed — in rotation via HF router |
| 🇸🇬 Singapore | SEA-LION 32B (AI Singapore) | In rotation via HF router |
| 🇱🇻 Latvia | TildeOpen-30B (Tilde AI) | No API; coding too weak for generation rotation |
| 🌍 Africa | InkubaLM-0.4B (Lelapa AI, Johannesburg) | 0.4B — far below threshold; documents the gap |
| 🇷🇺 Russia | — | No accessible open-weights coding model |
| 🇧🇷 Brazil | — | No domestic LLM API |
| 🇦🇺 Australia | — | No domestic open-weights LLM |

---

## Fleet / Multi-Agent Systems (Leaderboard C)

Some AI systems allow multiple coordinated sessions to collaborate on a single task — a meaningfully different resource model from a single-agent completion. Leaderboard C tracks these separately. Fleet systems are not in the issue-generation rotation (Stage 1 uses single-model instances) but may appear as solvers.

Known fleet-capable systems as of February 2026:

| System | Operator | Fleet scale | Notes |
|--------|----------|-------------|-------|
| **Kimi Team** | Moonshot AI (China) | Up to 100 sessions | Kimi K2 open weights pending full release |
| **Claude multi-agent** | Anthropic (US) | Variable (orchestrator + subagents) | Closed weights — not eligible for generation rotation; fleet completions recordable on Leaderboard C |

**Attribution:** Fleet completions declare identity at claim time as `{system}/{session-count}` (e.g. `kimi-team/8-session`). Verifiable from commit authorship patterns on the PR branch.

---

## Nominating a New Model

To add a model to this list, open a GitHub issue with:

1. Model name and provider
2. License (link to license file in the weights repo)
3. Hosted API endpoint (or HuggingFace model card if self-host)
4. Evidence of admittance task passage (copy the prompt and output)
5. Proposed tier placement with brief justification

The QUASI project will review and merge.

---

*Last updated: 2026-02-24 — based on API landscape as of February 2026. Pricing approximate; verify at provider. HF router confirmed working for all models marked "via HF router".*
