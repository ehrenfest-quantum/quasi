// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Model rotation logic: fair assignment, provider diversity, anti-collusion.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::config::{get_provider, ROTATION};
use crate::types::{Role, RotationEntry};

/// Return `true` if the given provider has its API key set in the environment.
pub fn provider_has_key(provider_id: &str) -> bool {
    match get_provider(provider_id) {
        Some(p) => {
            let val = std::env::var(p.env_var).unwrap_or_default();
            !val.is_empty()
        }
        None => false,
    }
}

/// Return all `RotationEntry` items whose provider's API key is available
/// and which support the given `role`.
pub fn eligible_for_role(role: &Role) -> Vec<&'static RotationEntry> {
    ROTATION
        .iter()
        .filter(|e| e.roles.contains(role) && provider_has_key(e.provider))
        .collect()
}

/// Pick the next model for a given role, respecting:
/// 1. The model must support the requested role.
/// 2. The model's provider must have its API key configured.
/// 3. The model must not appear in `exclude` (anti-collusion).
/// 4. Prefer the model with fewest assignments for this role (fair rotation).
/// 5. De-prioritise the provider used in the last call (load spreading).
///
/// On success returns a `&'static RotationEntry`.
pub fn pick_model(
    role: &Role,
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
) -> Result<&'static RotationEntry> {
    // Step 1: get all eligible candidates for this role.
    let candidates: Vec<&'static RotationEntry> = eligible_for_role(role)
        .into_iter()
        .filter(|e| !exclude.contains(&e.id))
        .collect();

    if candidates.is_empty() {
        return Err(anyhow!(
            "No eligible models for role {role} after exclusions"
        ));
    }

    // Step 2: sort by (count, same_provider_penalty, rotation_index).
    let rotation_index = |id: &str| -> usize {
        ROTATION.iter().position(|e| e.id == id).unwrap_or(usize::MAX)
    };

    let mut sorted = candidates;
    sorted.sort_by_key(|e| {
        let count = counts.get(e.id).copied().unwrap_or(0);
        let penalty: u32 = if last_provider == Some(e.provider) { 1 } else { 0 };
        let idx = rotation_index(e.id);
        (count, penalty, idx)
    });

    Ok(sorted[0])
}
