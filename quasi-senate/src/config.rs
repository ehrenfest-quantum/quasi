// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Static configuration: provider map, model rotation, capability ladder.
//!
//! Ported from quasi-agent/generate_issue.py.
//! 58 rotation entries across 10 providers — do not add models here without
//! a corresponding PR to docs/ELIGIBLE-MODELS.md.

use crate::types::{Role, RotationEntry};

// ── Providers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Provider {
    /// Chat completions endpoint (OpenAI-compatible /v1/chat/completions)
    pub url: &'static str,
    /// Environment variable holding the API key
    pub env_var: &'static str,
    /// Extra headers beyond Authorization and Content-Type
    pub extra_headers: &'static [(&'static str, &'static str)],
    /// Response header whose value should equal the requested model ID (anti-masking)
    pub verify_header: Option<&'static str>,
    /// Timeout in seconds (HuggingFace needs 600s; others 120s)
    pub timeout_secs: u64,
}

pub const PROVIDERS: &[(&str, Provider)] = &[
    (
        "openrouter",
        Provider {
            url: "https://openrouter.ai/api/v1/chat/completions",
            env_var: "OPENROUTER_API_KEY",
            extra_headers: &[
                ("HTTP-Referer", "https://quasi.arvak.io"),
                ("X-Title", "QUASI Pauli-Test Senate Loop"),
            ],
            verify_header: Some("x-finalized-model"),
            timeout_secs: 120,
        },
    ),
    (
        "sarvam",
        Provider {
            url: "https://api.sarvam.ai/v1/chat/completions",
            env_var: "SARVAM_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "mistral",
        Provider {
            url: "https://api.mistral.ai/v1/chat/completions",
            env_var: "MISTRAL_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "huggingface",
        Provider {
            url: "https://router.huggingface.co/v1/chat/completions",
            env_var: "HF_TOKEN",
            // User-Agent required — HF router proxies through Cloudflare-protected
            // backends that block Python's default urllib user agent.
            extra_headers: &[("User-Agent", "quasi-agent/1.0 (https://quasi.arvak.io)")],
            verify_header: None,
            timeout_secs: 600,
        },
    ),
    (
        "groq",
        Provider {
            url: "https://api.groq.com/openai/v1/chat/completions",
            env_var: "GROQ_API_KEY",
            // User-Agent required — Groq's Cloudflare layer blocks Python's default urllib UA.
            extra_headers: &[("User-Agent", "quasi-agent/1.0 (https://quasi.arvak.io)")],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "fireworks",
        Provider {
            url: "https://api.fireworks.ai/inference/v1/chat/completions",
            env_var: "FIREWORKS_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "swissai",
        Provider {
            url: "https://api.research.computer/v1/chat/completions",
            env_var: "CSCS_SERVING_API",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "together",
        Provider {
            url: "https://api.together.xyz/v1/chat/completions",
            env_var: "TOGETHER_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
    (
        "cerebras",
        Provider {
            // WSE is fast — short timeout is fine
            url: "https://api.cerebras.ai/v1/chat/completions",
            env_var: "CEREBRAS_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 60,
        },
    ),
    (
        "deepinfra",
        Provider {
            url: "https://api.deepinfra.com/v1/openai/chat/completions",
            env_var: "DEEPINFRA_API_KEY",
            extra_headers: &[],
            verify_header: None,
            timeout_secs: 120,
        },
    ),
];

pub fn get_provider(name: &str) -> Option<&'static Provider> {
    PROVIDERS
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

// ── Rotation Roster ───────────────────────────────────────────────────────────
//
// Role assignment rationale (from quasi-senate-loop.md §2):
//   - Reasoning specialists → A1Council, A3Gate, B2Reviewer
//   - Coding specialists    → A2Drafter, B1Solver
//   - General / regional    → A2Drafter, B1Solver (default)
//
// Cost preference: prefer Groq (free tier) and HuggingFace (free, rate-limited)
// for review roles (small token budgets); flagship OpenRouter models for B.1
// solving (8K output needed). Fireworks is pay-per-token.

// Role constant slices
const REASONING_ROLES: &[Role] = &[Role::A1Council, Role::A2Drafter, Role::A3Gate, Role::B2Reviewer];
const CODING_ROLES: &[Role] = &[Role::A2Drafter, Role::B1Solver];
const REVIEW_ROLES: &[Role] = &[Role::A1Council, Role::A3Gate, Role::B2Reviewer];
const DEFAULT_ROLES: &[Role] = &[Role::A2Drafter, Role::B1Solver];
const ALL_ROLES: &[Role] = &[Role::A1Council, Role::A2Drafter, Role::A3Gate, Role::B1Solver, Role::B2Reviewer];

pub const ROTATION: &[RotationEntry] = &[
    // ── Tier 1 — Strong coding (OpenRouter) ──────────────────────────────────
    RotationEntry {
        id: "deepseek-v3",
        model: "deepseek/deepseek-chat-v3-0324",
        provider: "openrouter",
        license: "MIT",
        origin: "China / DeepSeek",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepseek-v3-hf removed 2026-03-01: 0% approval rate on B1/A2, avg 45min latency on HuggingFace
    // (systematic HF routing slowness + model produces non-JSON output under coding prompts)
    RotationEntry {
        id: "deepseek-r1",
        model: "deepseek/deepseek-r1",
        provider: "openrouter",
        license: "MIT",
        origin: "China / DeepSeek",
        roles: REASONING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        // deepseek-r1-distill-llama-70b was decommissioned from Groq 2026-03
        // replaced with kimi-k2 (strong reasoning, free tier, Groq LPU)
        id: "kimi-k2-groq",
        model: "moonshotai/kimi-k2-instruct",
        provider: "groq",
        license: "Modified MIT",
        origin: "China / Moonshot AI",
        roles: REASONING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen3-coder",
        model: "qwen/qwen3-coder",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "China / Alibaba",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "llama4",
        model: "meta-llama/llama-4-maverick",
        provider: "openrouter",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // llama4-maverick-instruct-basic was removed from Fireworks 2026-03 → entry dropped
    // ── Tier 1 — Groq (FREE TIER — preferred for review roles) ───────────────
    // llama4-scout removed 2026-03-01: systematically produces Python string concat in JSON
    // edits (e.g. "replace": "def main():" + "\n\t...") causing 100% parse failure on B1 solver
    RotationEntry {
        id: "llama3.3",
        model: "meta-llama/llama-3.3-70b-instruct",
        provider: "openrouter",
        license: "Llama Community",
        origin: "US / Meta",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "llama3.3-groq",
        model: "llama-3.3-70b-versatile",
        provider: "groq",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "llama3.3-hf",
        model: "meta-llama/Llama-3.3-70B-Instruct",
        provider: "huggingface",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        // llama-4-maverick on Groq LPU — benchmark vs. OpenRouter/Fireworks variants
        id: "llama4-maverick-groq",
        model: "meta-llama/llama-4-maverick-17b-128e-instruct",
        provider: "groq",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        // OpenAI GPT-OSS 120B on Groq LPU — headline benchmark: custom silicon vs. GPU
        id: "gpt-oss-120b-groq",
        model: "openai/gpt-oss-120b",
        provider: "groq",
        license: "Open",
        origin: "US / OpenAI",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 2 — EU / competitive coding ─────────────────────────────────────
    RotationEntry {
        id: "mistral-small",
        model: "mistralai/mistral-small-3.1-24b-instruct",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "mistral-small-native",
        model: "mistral-small-2503",
        provider: "mistral",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "mistral-nemo",
        model: "mistralai/mistral-nemo",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "mistral-nemo-hf",
        model: "mistralai/Mistral-Nemo-Instruct-2407",
        provider: "huggingface",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 1 — HuggingFace (FREE, rate-limited) ─────────────────────────────
    RotationEntry {
        id: "kimi-k2",
        model: "moonshotai/Kimi-K2-Instruct",
        provider: "huggingface",
        license: "Modified MIT",
        origin: "China / Moonshot AI",
        // kimi-k2 is strong on reasoning — good for review roles
        roles: REASONING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "glm-4.7",
        model: "zai-org/GLM-4.7",
        provider: "huggingface",
        license: "MIT",
        origin: "China / Zhipu AI",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 2 — EU / competitive coding (HuggingFace) ───────────────────────
    RotationEntry {
        id: "eurollm-22b",
        model: "utter-project/EuroLLM-22B-Instruct-2512",
        provider: "huggingface",
        license: "Apache-2.0",
        origin: "EU consortium / Unbabel (Portugal)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "olmo-32b",
        model: "allenai/olmo-3.1-32b-instruct",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "US / Allen AI (fully open)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 3 — Regional participation ──────────────────────────────────────
    RotationEntry {
        id: "sarvam-m",
        model: "sarvam-m",
        provider: "sarvam",
        license: "Open",
        origin: "India / Sarvam AI",
        roles: DEFAULT_ROLES,
        max_tokens: Some(1500),
        max_context: None,
    },
    RotationEntry {
        id: "jamba",
        model: "ai21/jamba-large-1.7",
        provider: "openrouter",
        license: "Jamba Open",
        origin: "Israel / AI21",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "dicta",
        model: "dicta-il/DictaLM-3.0-24B-Thinking",
        provider: "huggingface",
        license: "Apache-2.0",
        origin: "Israel / Dicta (Bar-Ilan University)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "swallow-70b",
        model: "tokyotech-llm/Llama-3.3-Swallow-70B-Instruct-v0.4",
        provider: "huggingface",
        license: "Llama 3.3 Community",
        origin: "Japan / Tokyo Institute of Technology",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "sea-lion",
        model: "aisingapore/Qwen-SEA-LION-v4-32B-IT",
        provider: "huggingface",
        license: "Apache-2.0",
        origin: "Singapore / AI Singapore",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "ernie-4.5",
        model: "baidu/ernie-4.5-21b-a3b",
        provider: "openrouter",
        license: "ERNIE Open",
        origin: "China / Baidu",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "apertus",
        model: "swiss-ai/Apertus-70B-Instruct-2509",
        provider: "huggingface",
        license: "Fully open",
        origin: "Switzerland / ETH Zurich + EPFL + CSCS",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 1 additions — strong reasoning (OpenRouter) ─────────────────────
    RotationEntry {
        id: "qwq-32b",
        model: "qwen/qwq-32b",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "China / Alibaba (Qwen — reasoning model)",
        roles: REASONING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Tier 1 — Groq (FREE TIER) ─────────────────────────────────────────────
    RotationEntry {
        id: "qwen3-32b",
        model: "qwen/qwen3-32b",
        provider: "groq",
        license: "Apache-2.0",
        origin: "China / Alibaba (Qwen3 dense 32B)",
        // qwen3-32b on groq is free and has reasoning capability
        roles: ALL_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen3-32b-or",
        model: "qwen/qwen3-32b",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "China / Alibaba (Qwen3 dense 32B)",
        roles: REASONING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen3-30b",
        model: "qwen/qwen3-30b-a3b-instruct-2507",
        provider: "openrouter",
        license: "Apache-2.0",
        origin: "China / Alibaba (Qwen3 MoE, 30B total / 3B active)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── HuggingFace (FREE) ────────────────────────────────────────────────────
    RotationEntry {
        id: "gemma-3-27b",
        model: "google/gemma-3-27b-it",
        provider: "huggingface",
        license: "Gemma",
        origin: "US / Google DeepMind",
        // gemma-3-27b is a solid reasoning model on HF (free)
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "gemma-3-27b-or",
        model: "google/gemma-3-27b-it",
        provider: "openrouter",
        license: "Gemma",
        origin: "US / Google DeepMind",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "command-a",
        model: "cohere/command-a",
        provider: "openrouter",
        license: "CC-BY-NC-4.0",
        origin: "Canada / Cohere (Toronto)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── OpenRouter additions ──────────────────────────────────────────────────
    RotationEntry {
        id: "phi-4",
        model: "microsoft/phi-4",
        provider: "openrouter",
        license: "MIT",
        origin: "US / Microsoft Research",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: Some(24000),
    },
    RotationEntry {
        id: "phi-4-hf",
        model: "microsoft/phi-4",
        provider: "huggingface",
        license: "MIT",
        origin: "US / Microsoft Research",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: Some(24000),
    },
    RotationEntry {
        id: "nemotron-70b",
        model: "nvidia/llama-3.1-nemotron-70b-instruct",
        provider: "openrouter",
        license: "NVIDIA Open Model",
        origin: "US / NVIDIA Research",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "hermes-3",
        model: "nousresearch/hermes-3-llama-3.1-70b",
        provider: "openrouter",
        license: "Llama Community",
        origin: "US / Nous Research",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen2.5-72b",
        model: "qwen/qwen-2.5-72b-instruct",
        provider: "openrouter",
        license: "Qwen Community",
        origin: "China / Alibaba (general, not code-specific)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen2.5-72b-hf",
        model: "Qwen/Qwen2.5-72B-Instruct",
        provider: "huggingface",
        license: "Qwen Community",
        origin: "China / Alibaba (general, not code-specific)",
        roles: DEFAULT_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "gemma-3-12b",
        model: "google/gemma-3-12b-it",
        provider: "openrouter",
        license: "Gemma",
        origin: "US / Google DeepMind",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── HuggingFace additions (FREE) ──────────────────────────────────────────
    RotationEntry {
        id: "qwen2.5-7b",
        model: "qwen/qwen-2.5-7b-instruct",
        provider: "openrouter",
        license: "Qwen Community",
        origin: "China / Alibaba (smaller model, L0 tasks)",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Groq additions — Tier 3 regional (FREE TIER) ──────────────────────────
    RotationEntry {
        id: "allam-2",
        model: "allam-2-7b",
        provider: "groq",
        license: "Apache-2.0",
        origin: "Saudi Arabia / SDAIA",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── Fireworks additions — Tier 1 (pay-per-token) ──────────────────────────
    RotationEntry {
        id: "cogito-671b",
        model: "accounts/cogito/models/cogito-671b-v2-p1",
        provider: "fireworks",
        license: "MIT",
        origin: "US / Deep Cogito",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "minimax-m2",
        model: "accounts/fireworks/models/minimax-m2p1",
        provider: "fireworks",
        license: "Modified MIT",
        origin: "China / MiniMax (Shanghai)",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // ── High-priority multi-provider overlaps (Together AI / Cerebras / DeepInfra / Fireworks) ──
    // llama3.3 × 4 providers (benchmark: custom-silicon vs. optimised-GPU vs. aggregator)
    RotationEntry {
        id: "llama3.3-together",
        model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        provider: "together",
        license: "Llama Community",
        origin: "US / Meta",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "llama3.3-cerebras",
        model: "llama-3.3-70b",
        provider: "cerebras",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    RotationEntry {
        id: "llama3.3-fireworks",
        model: "accounts/fireworks/models/llama-v3p3-70b-instruct",
        provider: "fireworks",
        license: "Llama Community",
        origin: "US / Meta",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepseek-r1 × 3 new providers
    RotationEntry {
        id: "deepseek-r1-together",
        model: "deepseek-ai/DeepSeek-R1",
        provider: "together",
        license: "MIT",
        origin: "China / DeepSeek",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepseek-r1 was removed from Fireworks 2026-03 → entry dropped
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    // deepseek-v3 × 3 new providers
    RotationEntry {
        id: "deepseek-v3-together",
        model: "deepseek-ai/DeepSeek-V3",
        provider: "together",
        license: "MIT",
        origin: "China / DeepSeek",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        // deepseek-v3 → deepseek-v3p2 (model string updated 2026-03)
        id: "deepseek-v3-fireworks",
        model: "accounts/fireworks/models/deepseek-v3p2",
        provider: "fireworks",
        license: "MIT",
        origin: "China / DeepSeek",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    // qwen2.5-72b × 2 new providers
    RotationEntry {
        id: "qwen2.5-72b-together",
        model: "Qwen/Qwen2.5-72B-Instruct-Turbo",
        provider: "together",
        license: "Qwen Community",
        origin: "China / Alibaba",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    // qwen3-32b × 3 providers (headline: Groq LPU vs. Cerebras WSE head-to-head)
    RotationEntry {
        id: "qwen3-32b-together",
        model: "Qwen/Qwen3-32B",
        provider: "together",
        license: "Apache-2.0",
        origin: "China / Alibaba",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "qwen3-32b-cerebras",
        model: "qwen-3-32b",
        provider: "cerebras",
        license: "Apache-2.0",
        origin: "China / Alibaba",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    // gemma-3-27b × 2 new providers
    RotationEntry {
        id: "gemma-3-27b-together",
        model: "google/gemma-3-27b-it",
        provider: "together",
        license: "Gemma",
        origin: "US / Google DeepMind",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // deepinfra: no credits yet — entry commented out (re-enable when credits arrive)
    // ── Medium-priority overlaps ───────────────────────────────────────────────
    RotationEntry {
        id: "mistral-small-mistral",
        model: "mistral-small-latest",
        provider: "mistral",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "mistral-nemo-mistral",
        model: "open-mistral-nemo",
        provider: "mistral",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "mistral-nemo-together",
        model: "mistralai/Mistral-Nemo-Instruct-2407",
        provider: "together",
        license: "Apache-2.0",
        origin: "France / Mistral",
        roles: CODING_ROLES,
        max_tokens: None,
        max_context: None,
    },
    RotationEntry {
        id: "phi-4-together",
        model: "microsoft/phi-4",
        provider: "together",
        license: "MIT",
        origin: "US / Microsoft Research",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: Some(24000),
    },
    RotationEntry {
        id: "llama4-scout-together",
        model: "meta-llama/Llama-4-Scout-17B-16E-Instruct",
        provider: "together",
        license: "Llama Community",
        origin: "US / Meta",
        roles: REVIEW_ROLES,
        max_tokens: None,
        max_context: None,
    },
    // llama4-scout-instruct-basic was removed from Fireworks 2026-03 → entry dropped
];

// ── Capability Ladder ──────────────────────────────────────────────────────────

pub const LEVEL_NAMES: &[(u8, &str)] = &[
    (0, "L0 — Interfaces & Contracts (HAL Contract bindings, ActivityPub API endpoints, CLI UX, quasi-board task lifecycle)"),
    (1, "L1 — Language Foundations (Ehrenfest syntax, parser, AST, type system, CBOR schema)"),
    (2, "L2 — Compiler / Afana (ZX-IR generation, rewriting rules, QASM3 output, optimisation passes)"),
    (3, "L3 — Hardware Backends (IBM/IQM adapters, HAL Contract execution, error mitigation, shot noise)"),
    (4, "L4 — Turing-Complete Runtime (quantum memory model, classical control flow, variational loops)"),
];

pub fn level_name(level: u8) -> &'static str {
    LEVEL_NAMES
        .iter()
        .find(|(l, _)| *l == level)
        .map(|(_, n)| *n)
        .unwrap_or("Unknown level")
}

/// Labels available for issue proposals (intentionally excludes 'infrastructure' and 'docs').
pub const LABEL_TAXONOMY: &str =
    "compiler · specification · core · agent-ux · good-first-issue";

// ── Verification ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_has_58_models() {
        // 60 baseline
        // - deepseek-v3-hf: 0% approval, 45min avg latency on HuggingFace (removed 2026-03-01)
        // - llama4-scout: systematic JSON parse failures on B1 solver (removed 2026-03-01)
        // = 58
        assert_eq!(ROTATION.len(), 58, "Expected 58 models in ROTATION");
    }

    #[test]
    fn all_rotation_providers_exist() {
        for entry in ROTATION {
            assert!(
                get_provider(entry.provider).is_some(),
                "Model {} has unknown provider '{}'",
                entry.id,
                entry.provider
            );
        }
    }

    #[test]
    fn all_roles_nonempty() {
        for entry in ROTATION {
            assert!(!entry.roles.is_empty(), "Model {} has no roles", entry.id);
        }
    }
}
