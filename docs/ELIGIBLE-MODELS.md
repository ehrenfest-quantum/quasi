# Eligible Models — Pauli-Test Issue Generation

Models approved for the Stage 1 (issue generation) rotation and Leaderboard B (autonomous completions). Maintained by the community — see `docs/ISSUE-GENERATION.md` for nomination process.

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
| **Qwen2.5-Coder-32B-Instruct** | China (Alibaba / Qwen team) | Qwen license | `qwen/qwen-2.5-coder-32b-instruct` | $0.07 | $0.16 |
| **Llama 4 Maverick** | US (Meta) | Llama Community | `meta-llama/llama-4-maverick` | $0.17 | $0.60 |
| **Llama 3.3 70B Instruct** | US (Meta) | Llama 3.3 Community | `meta-llama/llama-3.3-70b-instruct` | $0.05 | $0.08 |

**Notes:**
- DeepSeek MIT license is the cleanest open-weights license in this tier — no usage restrictions.
- Qwen's license is permissive but requires attribution; verify before commercial use.
- Llama Community License permits commercial use above 700M MAU threshold only for enterprises; free for this benchmark use case.
- All five are available via OpenRouter with pinnable version strings.

---

## Tier 2 — Competitive Coding, EU-Origin

European models. Mistral is the only EU provider with production-grade coding capability and open weights.

| Model | Origin | License | Versioned ID (OpenRouter / provider) | Approx. cost/1M in | Approx. cost/1M out |
|-------|--------|---------|--------------------------------------|--------------------|--------------------|
| **Mistral Small 3.1** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-small-3.1-24b-instruct` | $0.10 | $0.30 |
| **Mistral Nemo** | France (Mistral AI) | Apache 2.0 | `mistralai/mistral-nemo` | $0.02 | $0.08 |

**Notes:**
- Codestral (Mistral's dedicated code model) is not open weights — excluded.
- Mistral Large 2 is not open weights — excluded.
- Mistral Small 3.1 and Nemo are Apache 2.0: the strongest EU-origin open-weights models with public APIs.
- Both available via `api.mistral.ai` and OpenRouter.

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

**Notes:**
- Sarvam-30B/105B: released February 18, 2026. India's only models with a public API and open weights. Competitive on math/reasoning vs Gemma 27B class. Not yet benchmarked on quantum/compiler tasks.
- Falcon 3 10B: Apache 2.0, genuinely free. Coding capability is weak; representative of the UAE open-source effort. Good for L0.
- EXAONE 3.5: LG AI Research's 32B model, competitive on reasoning benchmarks. Self-host only — requires infrastructure. Worth admitting when hosted by a third party.
- GLM-4-9B: Zhipu AI open weights on HuggingFace. Chinese-English bilingual. International API portal exists but pricing/ToS unclear from EU — self-host is cleaner.
- Jamba 1.5 Mini: Apache 2.0, small but architecturally novel (SSM/Mamba hybrid).

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
| 🇺🇸 US | Llama 4 Maverick, Llama 3.3 70B | No gap — strong coverage |
| 🇨🇳 China | DeepSeek-V3, DeepSeek-R1, Qwen2.5-Coder | No gap — Tier 1 |
| 🇫🇷 Europe | Mistral Small 3.1, Mistral Nemo | Codestral excluded (closed weights) |
| 🇩🇪 Germany | Aleph Alpha / Pharia — no public API | Real gap |
| 🇮🇳 India | Sarvam-30B, Sarvam-105B | Coding capability still maturing |
| 🇦🇪 UAE | Falcon 3 10B | Weak coding; participation-level only |
| 🇰🇷 Korea | EXAONE 3.5 | No public API yet — self-host only |
| 🇨🇦 Canada | — | Cohere Command R open model (Aya) is weak on coding |
| 🇮🇱 Israel | Jamba 1.5 Mini | Small model; participation-level |
| 🇷🇺 Russia | — | No accessible open-weights coding model from EU |
| 🇯🇵 Japan | — | No production open-weights model with English coding |
| 🇧🇷 Brazil | — | No domestic LLM API |

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
