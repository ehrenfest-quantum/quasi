// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Configuration: provider map, model rotation (TOML-loaded), capability ladder.
//!
//! The rotation roster is loaded from an external TOML file at startup.
//! Falls back to the embedded default (`rotation.toml` compiled into the binary).
//! Edit `/home/vops/quasi-senate-rotation.toml` on the server to add/remove
//! models without recompiling.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::Deserialize;
use tracing::{info, warn};

use crate::types::RotationEntry;

// ── Providers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Provider {
    /// Chat completions endpoint (OpenAI-compatible /v1/chat/completions).
    /// Empty string is allowed only when `url_env_var` is `Some(_)` — the
    /// effective URL is then read from the environment at call time.
    pub url: &'static str,
    /// Environment variable holding the API key. Empty string means the
    /// provider needs no API key (e.g. local Ollama on a private tailnet).
    pub env_var: &'static str,
    /// Extra headers beyond Authorization and Content-Type
    pub extra_headers: &'static [(&'static str, &'static str)],
    /// Response header whose value should equal the requested model ID (anti-masking)
    pub verify_header: Option<&'static str>,
    /// Timeout in seconds (HuggingFace needs 600s; others 120s)
    pub timeout_secs: u64,
    /// Optional environment variable holding a runtime-resolved chat
    /// completions URL. Used for self-hosted backends whose endpoint is
    /// not known at compile time (e.g. Ollama on a tailnet hostname).
    pub url_env_var: Option<&'static str>,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
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
            url_env_var: None,
        },
    ),
    (
        // Self-hosted Ollama backend reachable on a private tailnet.
        // The endpoint is read from $OLLAMA_URL at call time (e.g.
        // http://mac-studio:11434/v1/chat/completions). No API key.
        "ollama",
        Provider {
            url: "",
            env_var: "",
            extra_headers: &[],
            verify_header: None,
            // Local 30B-class models stream slowly; allow long generations.
            timeout_secs: 900,
            url_env_var: Some("OLLAMA_URL"),
        },
    ),
];

/// Reduce a provider-qualified model string to a coarse family key.
///
/// `zai-org/GLM-5.2`, `z-ai/glm-5.2` and `accounts/fireworks/models/glm-5p2`
/// are the same underlying model served three ways. Callers use this to avoid
/// treating a re-served model as an independent choice — both for judge
/// disjointness and for retrying with a genuinely different model.
pub fn model_family(model: &str) -> String {
    let tail = model.rsplit('/').next().unwrap_or(model).to_lowercase();

    // Join a version number split by a separator, so "5.2" and Fireworks'
    // "5p2" convention both collapse to "52". Without this, GLM-5.2 served by
    // Together ("glm-5.2") and by Fireworks ("glm-5p2") look like different
    // families and a retry treats the second as a fresh opinion.
    let bytes: Vec<char> = tail.chars().collect();
    let mut joined = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let is_version_sep = i > 0
            && i + 1 < bytes.len()
            && bytes[i - 1].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && (bytes[i] == '.' || bytes[i] == 'p');
        if !is_version_sep {
            joined.push(bytes[i]);
        }
        i += 1;
    }

    let alnum: String = joined
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    alnum.split_whitespace().take(2).collect::<Vec<_>>().concat()
}

pub fn get_provider(name: &str) -> Option<&'static Provider> {
    PROVIDERS
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

// ── Rotation Roster ───────────────────────────────────────────────────────────
//
// Loaded at startup from external TOML (server path) or embedded default.
// To add/remove models: edit rotation.toml and restart the service.

/// External TOML path checked at startup (before falling back to embedded).
const EXTERNAL_ROTATION_PATH: &str = "/home/vops/quasi-senate-rotation.toml";

/// Embedded default compiled into the binary.
const EMBEDDED_ROTATION_TOML: &str = include_str!("../rotation.toml");

/// TOML document shape: `[[rotation]]` array.
#[derive(Deserialize)]
struct RotationFile {
    rotation: Vec<RotationEntry>,
}

/// Stores the leaked rotation slice for `&'static` access.
static ROTATION: OnceLock<&'static [RotationEntry]> = OnceLock::new();

/// Initialize the rotation roster. Must be called once at startup before
/// any code accesses `rotation()`.
///
/// Load order:
///   1. External file at `EXTERNAL_ROTATION_PATH` (hand-editable on server)
///   2. Embedded default (`rotation.toml` compiled into the binary)
///
/// Panics on validation failure (duplicate IDs, unknown providers, empty roles).
pub fn init_rotation() {
    ROTATION.get_or_init(|| {
        let entries = load_rotation();
        validate_rotation(&entries);
        // Leak into 'static so all consumers keep their &'static RotationEntry signatures.
        let leaked: &'static [RotationEntry] = Box::leak(entries.into_boxed_slice());
        info!(count = leaked.len(), "Rotation roster loaded");
        leaked
    });
}

/// Access the rotation roster. Panics if `init_rotation()` was not called.
pub fn rotation() -> &'static [RotationEntry] {
    ROTATION.get().expect("init_rotation() must be called before rotation()")
}

fn load_rotation() -> Vec<RotationEntry> {
    // Try external file first.
    match std::fs::read_to_string(EXTERNAL_ROTATION_PATH) {
        Ok(content) => {
            info!(path = EXTERNAL_ROTATION_PATH, "Loading rotation from external TOML");
            match toml::from_str::<RotationFile>(&content) {
                Ok(file) => return file.rotation,
                Err(e) => {
                    warn!(
                        path = EXTERNAL_ROTATION_PATH,
                        error = %e,
                        "Failed to parse external TOML — falling back to embedded default"
                    );
                }
            }
        }
        Err(_) => {
            info!("No external rotation file — using embedded default");
        }
    }

    // Fallback: embedded default.
    toml::from_str::<RotationFile>(EMBEDDED_ROTATION_TOML)
        .expect("Embedded rotation.toml must be valid")
        .rotation
}

fn validate_rotation(entries: &[RotationEntry]) {
    // 1. No duplicate IDs.
    let mut seen = HashSet::new();
    for entry in entries {
        if !seen.insert(&entry.id) {
            panic!("Duplicate rotation ID: '{}'", entry.id);
        }
    }

    // 2. All providers must exist in the compiled PROVIDERS map.
    for entry in entries {
        if get_provider(&entry.provider).is_none() {
            panic!(
                "Model '{}' references unknown provider '{}'",
                entry.id, entry.provider
            );
        }
    }

    // 3. Every entry must have at least one role (unless quarantined).
    for entry in entries {
        if entry.roles.is_empty() && !entry.quarantined {
            panic!("Model '{}' has no roles assigned", entry.id);
        }
    }
}

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
    use crate::types::Role;
    use super::model_family;

    fn ensure_init() {
        init_rotation();
    }

    #[test]
    fn rotation_has_at_least_40_models() {
        ensure_init();
        assert!(rotation().len() >= 40, "Expected at least 40 models in rotation, got {}", rotation().len());
    }

    #[test]
    fn all_rotation_providers_exist() {
        ensure_init();
        for entry in rotation() {
            assert!(
                get_provider(&entry.provider).is_some(),
                "Model {} has unknown provider '{}'",
                entry.id,
                entry.provider
            );
        }
    }

    #[test]
    fn all_roles_nonempty() {
        ensure_init();
        for entry in rotation() {
            assert!(!entry.roles.is_empty(), "Model {} has no roles", entry.id);
        }
    }

    /// The Werner integrity guarantee, enforced rather than documented.
    ///
    /// A judge must be drawn from a pool the generator can never draw from, so
    /// no model may hold `werner_judge` alongside a generator role. Without
    /// this test the property erodes silently: before it existed, 61 of 93
    /// reviewer-capable entries were also generator-capable, and the only
    /// anti-collusion was excluding the drafter's own model by id.
    #[test]
    fn werner_judges_are_disjoint_from_generators() {
        ensure_init();
        const GENERATOR_ROLES: [Role; 2] = [Role::A2Drafter, Role::B1Solver];
        let mut violations: Vec<String> = Vec::new();
        for entry in rotation() {
            if !entry.roles.contains(&Role::WernerJudge) {
                continue;
            }
            let also: Vec<String> = GENERATOR_ROLES
                .iter()
                .filter(|r| entry.roles.contains(r))
                .map(|r: &Role| r.to_string())
                .collect();
            if !also.is_empty() {
                violations.push(format!("{} also holds {}", entry.id, also.join(" + ")));
            }
        }
        assert!(
            violations.is_empty(),
            "werner_judge must be disjoint from generator roles, but: {}",
            violations.join("; ")
        );
    }


    /// Role-level disjointness is necessary but not sufficient: the same model
    /// served by two providers can sit on both sides under different ids.
    #[test]
    fn werner_judge_models_do_not_also_generate() {
        ensure_init();
        const GENERATOR_ROLES: [Role; 2] = [Role::A2Drafter, Role::B1Solver];
        let gen_families: HashSet<String> = rotation()
            .iter()
            .filter(|e| !e.quarantined && GENERATOR_ROLES.iter().any(|r| e.roles.contains(r)))
            .map(|e| model_family(&e.model))
            .collect();
        let mut violations: Vec<String> = Vec::new();
        for entry in rotation() {
            if entry.quarantined || !entry.roles.contains(&Role::WernerJudge) {
                continue;
            }
            let fam = model_family(&entry.model);
            if gen_families.contains(&fam) {
                violations.push(format!("{} ({} -> family '{}')", entry.id, entry.model, fam));
            }
        }
        assert!(
            violations.is_empty(),
            "these Werner judges share a model family with an active generator: {}",
            violations.join("; ")
        );
    }

    /// A judge pool that is empty in practice is the same failure as no pool at
    /// all — the reviewer errors out and the B-track stalls.
    #[test]
    fn werner_judge_pool_is_not_empty() {
        ensure_init();
        let judges = rotation()
            .iter()
            .filter(|e| !e.quarantined && e.roles.contains(&Role::WernerJudge))
            .count();
        assert!(
            judges > 0,
            "no active model holds the werner_judge role; the B-track reviewer cannot run"
        );
    }

    #[test]
    fn no_duplicate_ids() {
        ensure_init();
        let mut seen = HashSet::new();
        for entry in rotation() {
            assert!(seen.insert(&entry.id), "Duplicate rotation ID: '{}'", entry.id);
        }
    }

    #[test]
    fn embedded_toml_parses() {
        let file: RotationFile = toml::from_str(EMBEDDED_ROTATION_TOML)
            .expect("Embedded rotation.toml must parse");
        assert!(!file.rotation.is_empty());
    }
}
