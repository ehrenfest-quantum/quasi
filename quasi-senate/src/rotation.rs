// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Model rotation logic: fair assignment, provider diversity, anti-collusion.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::config::{get_provider, rotation};
use crate::types::{Role, RotationEntry};

/// Return `true` if the given provider is usable in the current environment.
///
/// A provider is "available" when:
///   - its `env_var` (API key) is set and non-empty (or empty `env_var` means
///     no key required, e.g. self-hosted Ollama on a private tailnet);
///   - if the provider resolves its URL from the environment, that URL var
///     is also set and non-empty.
pub fn provider_has_key(provider_id: &str) -> bool {
    let p = match get_provider(provider_id) {
        Some(p) => p,
        None => return false,
    };

    if !p.env_var.is_empty() {
        let val = std::env::var(p.env_var).unwrap_or_default();
        if val.trim().is_empty() {
            return false;
        }
    }

    if let Some(url_var) = p.url_env_var {
        let val = std::env::var(url_var).unwrap_or_default();
        if val.trim().is_empty() {
            return false;
        }
    }

    true
}

/// Return all `RotationEntry` items which support the given `role`, whose
/// provider's API key is available, and whose provider is currently live
/// (self-hosted endpoints are probed; see `availability`).
pub fn eligible_for_role(role: &Role) -> Vec<&'static RotationEntry> {
    eligible_from(rotation(), role)
}

fn eligible_from<'a>(entries: &'a [RotationEntry], role: &Role) -> Vec<&'a RotationEntry> {
    entries
        .iter()
        .filter(|e| {
            !e.quarantined
                && e.roles.contains(role)
                && provider_has_key(&e.provider)
                && crate::availability::is_provider_live(&e.provider)
        })
        .collect()
}

/// Pick the next model for a given role, respecting:
/// 1. The model must support the requested role.
/// 2. The model's provider must have its API key configured and be live.
/// 3. The model must not appear in `exclude` (anti-collusion).
/// 4. Prefer the lowest cost tier (free self-hosted models first).
/// 5. Prefer the model with fewest assignments for this role (fair rotation).
/// 6. De-prioritise the provider used in the last call (load spreading).
///
/// On success returns a `&'static RotationEntry`.
pub fn pick_model(
    role: &Role,
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
) -> Result<&'static RotationEntry> {
    pick_from(rotation(), role, exclude, counts, last_provider)
}

fn pick_from<'a>(
    entries: &'a [RotationEntry],
    role: &Role,
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
) -> Result<&'a RotationEntry> {
    // Step 1: get all eligible candidates for this role.
    //
    // Exclusion is by model FAMILY, not just by entry id. Excluding only the id
    // lets a retry pick the same model served by another provider, which is not
    // a second opinion — it is the first one again. Observed on issue #1115:
    // attempt 1 used glm-5.2-together (returned empty content), attempt 2 then
    // picked glm-5.2-fireworks, spending both attempts on GLM-5.2.
    let excluded_families: Vec<String> = entries
        .iter()
        .filter(|e| exclude.contains(&e.id.as_str()))
        .map(|e| crate::config::model_family(&e.model))
        .collect();
    let candidates: Vec<&RotationEntry> = eligible_from(entries, role)
        .into_iter()
        .filter(|e| !exclude.contains(&e.id.as_str()))
        .filter(|e| !excluded_families.contains(&crate::config::model_family(&e.model)))
        .collect();

    if candidates.is_empty() {
        return Err(anyhow!(
            "No eligible models for role {role} after exclusions"
        ));
    }

    // Step 2: sort by (cost_tier, count, same_provider_penalty, rotation_index).
    // Cost tier dominates: a live free model is always preferred, while the
    // existing fairness/diversity behaviour applies within a tier.
    let rotation_index =
        |id: &str| -> usize { entries.iter().position(|e| e.id == id).unwrap_or(usize::MAX) };

    let mut sorted = candidates;
    sorted.sort_by_key(|e| {
        let count = counts.get(&e.id).copied().unwrap_or(0);
        let penalty: u32 = if last_provider == Some(e.provider.as_str()) { 1 } else { 0 };
        let idx = rotation_index(&e.id);
        (e.cost_tier, count, penalty, idx)
    });

    Ok(sorted[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(id: &str, provider: &str, cost_tier: u8) -> RotationEntry {
        RotationEntry {
            id: id.to_string(),
            model: format!("{id}-model"),
            provider: provider.to_string(),
            license: "test".to_string(),
            origin: "test".to_string(),
            roles: vec![Role::B1Solver],
            quarantined: false,
            max_tokens: None,
            max_context: None,
            cost_tier,
        }
    }

    fn setup_env() {
        // Port 1 is never listening; the actual probe result is overridden
        // via the availability cache in each test.
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1/v1/chat/completions");
        std::env::set_var("GROQ_API_KEY", "test-key");
    }

    #[test]
    fn pick_prefers_free_tier_when_local_live() {
        let _guard = crate::availability::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        setup_env();
        crate::availability::force_provider_live("ollama");

        let entries = vec![
            test_entry("paid-model", "groq", 1),
            test_entry("free-model", "ollama", 0),
        ];
        let counts: HashMap<String, u32> = HashMap::new();
        let picked = pick_from(&entries, &Role::B1Solver, &[], &counts, None)
            .expect("a model should be eligible");
        assert_eq!(picked.id, "free-model");
    }

    /// Regression for issue #1115: excluding by id alone let the retry pick the
    /// same model from another provider, so both solve attempts went to GLM-5.2.
    #[test]
    fn retry_excludes_the_whole_model_family_not_just_the_id() {
        let _guard = crate::availability::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        setup_env();

        let mut a = test_entry("glm-together", "groq", 1);
        a.model = "zai-org/GLM-5.2".to_string();
        let mut b = test_entry("glm-fireworks", "groq", 1);
        b.model = "accounts/fireworks/models/glm-5p2".to_string();
        let mut c = test_entry("other-model", "groq", 1);
        c.model = "deepseek/deepseek-v4-flash".to_string();
        let entries = vec![a, b, c];
        let counts: HashMap<String, u32> = HashMap::new();

        // Exclude the first GLM entry, as the retry loop does after a failure.
        let picked = pick_from(&entries, &Role::B1Solver, &["glm-together"], &counts, None)
            .expect("a different model should remain");
        assert_eq!(
            picked.id, "other-model",
            "retry must not pick the same model family via another provider"
        );
    }

    #[test]
    fn pick_falls_back_to_paid_when_local_down() {
        let _guard = crate::availability::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        setup_env();
        crate::availability::mark_provider_down("ollama");

        let entries = vec![
            test_entry("paid-model", "groq", 1),
            test_entry("free-model", "ollama", 0),
        ];
        let counts: HashMap<String, u32> = HashMap::new();
        let picked = pick_from(&entries, &Role::B1Solver, &[], &counts, None)
            .expect("the paid model should still be eligible");
        assert_eq!(picked.id, "paid-model");
    }
}
