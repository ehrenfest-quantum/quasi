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

**Notes:**
- DeepSeek MIT license is the cleanest open-weights license in this tier — no usage restrictions.
- Qwen's license is permissive but requires attribution; verify before commercial use.
- Llama Community License permits commercial use above 700M MAU threshold only for enterprises; free for this benchmark use case.
- Qwen3-Coder replaces Qwen2.5-Coder-32B: `qwen/qwen-2.5-coder-32b-instruct` on OpenRouter has a confirmed truncation bug (finish_reason: None, response cut mid-token on certain content). Qwen3-Coder (`qwen/qwen3-coder`) routes correctly. Apache 2.0 license — an upgrade over the Qwen Community License.
- StarCoder2 is ServiceNow Research (Montréal) + BigCode (HuggingFace). 619 programming languages, 4 trillion tokens. Canada's best open-weights code model.
- All six are available via OpenRouter or HuggingFace with pinnable version strings.

---

## Tier 2 — Competitive Coding, EU-Origin

European models with coding capability and open weights.

| Model | Origin | License | Versioned ID (OpenRouter / provider) | Approx. cost/1M in | Approx. cost/1M out |
|-------|--------|---------|--------------------------------------|--------------------|--------------------|
| **Mistral Small 3.1** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-small-3.1-24b-instruct` | $0.10 | $0.30 |
| **Mistral Nemo** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-nemo` | $0.02 | $0.08 |
| **Viking-33B** | Finland (AMD Silo AI + TurkuNLP + HPLT) | Apache 2.0 | Self-host (HuggingFace) · Google Cloud Vertex AI | Free (self-host) | — |
| **Apertus-70B** | Switzerland (ETH Zurich + EPFL + CSCS) | Fully open | Swisscom API · self-host (HuggingFace) | Free (self-host) | — |

**Notes:**
- Codestral (Mistral's dedicated code model) is not open weights — excluded.
- Mistral Large 2 is not open weights — excluded.
- Mistral Small 3.1 and Nemo are Apache 2.0 with managed APIs via `api.mistral.ai` and OpenRouter.
- Viking-33B: Apache 2.0, trained on LUMI supercomputer. AMD acquired Silo AI (Finland) in 2024. Medium coding capability. No managed API currently — self-host or Google Cloud Vertex.
- Apertus-70B: The most transparent large model ever released — weights, training data (15T tokens), training code, and all checkpoints are public. ETH Zurich's "We trade speed for sunlight" principle. Medium coding capability (on par with Llama 3, 2024 class). Swisscom API available; self-host feasible.
- Neither Viking nor Apertus have OpenRouter IDs at time of writing. Add to generation rotation when a hosted API with pinnable version strings is available.

---

## Tier 3 — Regional Participation

Models from underrepresented regions. Coding capability is weaker, but participation is the point. Expected to start with L0 tasks.

| Model | Origin | License | Access | Approx. cost |
|-------|--------|---------|--------|--------------|
| **Sarvam-30B** | India (Sarvam AI, Bangalore) | Open (HuggingFace) | Sarvam developer API — free beta; self-hostable | Free (beta) |
| **Sarvam-105B** | India (Sarvam AI) | Open (HuggingFace) | Sarvam developer API — free beta | Free (beta) |
| **Falcon 3 10B** | UAE (TII, Abu Dhabi) | Apache 2.0 | AI71 platform — free; OpenRouter; self-hostable | Free |
| **EXAONE 3.5 32B** | South Korea (LG AI Research) | EXAONE Community | Self-hosted via HuggingFace (no public API currently) | Free (self-host) |
| **GLM-4-9B** | China (Zhipu AI) | Open (HuggingFace) | Self-hosted; limited international API at bigmodel.cn | Free (self-host) |
| **Jamba 1.5 Mini** | Israel (AI21 Labs) | Apache 2.0 | `ai21/jamba-1-5-mini` on OpenRouter | $0.20 | $0.40 |
| **TildeOpen-30B** | Latvia (Tilde AI) | CC-BY-4.0 | Self-host (HuggingFace) | Free |
| **EuroLLM-9B** | EU consortium (9 partners, lead: Lisbon) | Open | Self-host (HuggingFace) | Free |
| **InkubaLM-0.4B** | South Africa (Lelapa AI, Johannesburg) | Open | Self-host (HuggingFace) | Free |

**Notes:**
- Sarvam-30B/105B: released February 18, 2026. India's only models with a public API and open weights. Competitive on math/reasoning vs Gemma 27B class. Not yet benchmarked on quantum/compiler tasks.
- Falcon 3 10B: Apache 2.0, genuinely free. Coding capability is weak; representative of the UAE open-source effort. Good for L0.
- EXAONE 3.5: LG AI Research's 32B model, competitive on reasoning benchmarks. Self-host only — requires infrastructure. Worth admitting when hosted by a third party.
- GLM-4-9B: Zhipu AI open weights on HuggingFace. Chinese-English bilingual. International API portal exists but pricing/ToS unclear from EU — self-host is cleaner.
- Jamba 1.5 Mini: Apache 2.0, SSM/Mamba hybrid architecture. Israel's open-weights representative.
- TildeOpen-30B: CC-BY-4.0 (the most permissive license on this list), trained on EU public compute (LUMI + JUPITER supercomputers), all 34 EU languages. Coding capability weak. Not eligible for generation rotation; included for coverage documentation.
- EuroLLM-9B: EU-funded consortium of 9 partners. 24 official EU languages. Coding capability weak. Not eligible for generation rotation; included for coverage documentation.
- InkubaLM-0.4B: Africa's first open-weights model trained from scratch on African languages. 0.4B parameters — a Small Language Model, not competitive on coding. Included to document the African AI frontier and its current gap. Not eligible for generation rotation.

---

## Pending — Weights Partially Released

| Model | Origin | Status | Blocker |
|-------|--------|--------|---------|
| **Kimi K2** | China (Moonshot AI) | Partial weights released | Full weights not yet public; MoE architecture partially documented. Revisit when complete weights available. |
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
| Baidu ERNIE | Access wall + weights not publicly downloadable |
| ByteDance Doubao | China-only registration; weights not public |
| YandexGPT | Sanctions barrier from EU; weights not public |
| GigaChat | Registration wall; weights not public |
| HyperCLOVA X | Weights not fully open; EU access not streamlined |
| Krutrim-2 | Closed weights; pricing unclear |

---

## Coverage Map

| Region | Model(s) in rotation | Gap |
|--------|---------------------|-----|
| 🇺🇸 US | Llama 4 Maverick, Llama 3.3 70B, StarCoder2-15B (Montréal) | No gap — strong coverage |
| 🇨🇳 China | DeepSeek-V3, DeepSeek-R1, Qwen2.5-Coder | No gap — Tier 1 |
| 🇫🇷 France | Mistral Small 3.1, Mistral Nemo | Codestral excluded (closed weights) |
| 🇫🇮 Finland | Viking-33B (AMD Silo AI) | No managed API yet; self-host or Google Cloud |
| 🇨🇭 Switzerland | Apertus-70B (ETH Zurich + EPFL) | Swisscom API; no OpenRouter ID yet |
| 🇩🇪 Germany | Aleph Alpha / Pharia — no public API | Real gap |
| 🇮🇳 India | Sarvam-30B, Sarvam-105B | Coding capability still maturing |
| 🇦🇪 UAE | Falcon 3 10B | Weak coding; participation-level only |
| 🇰🇷 Korea | EXAONE 3.5 | No public API yet — self-host only |
| 🇨🇦 Canada | StarCoder2 (ServiceNow Research, Montréal) | Listed under US/BigCode; Montréal origin noted |
| 🇮🇱 Israel | Jamba 1.5 Mini | Small model; participation-level |
| 🇱🇻 Latvia | TildeOpen-30B (Tilde AI) | No API; coding too weak for generation rotation |
| 🇵🇹 Portugal | EuroLLM (EU consortium, Unbabel lead) | No API; coding too weak for generation rotation |
| 🌍 Africa | InkubaLM-0.4B (Lelapa AI, Johannesburg) | 0.4B — far below threshold; documents the gap |
| 🇷🇺 Russia | — | No accessible open-weights coding model from EU |
| 🇯🇵 Japan | — | No production open-weights model with English coding |
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

*Last updated: 2026-02-24 — based on API landscape as of February 2026. Pricing approximate; verify at provider.*
