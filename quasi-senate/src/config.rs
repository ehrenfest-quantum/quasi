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
