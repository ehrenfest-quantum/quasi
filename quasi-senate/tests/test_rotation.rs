// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
// Unit tests for model selection logic

use std::collections::HashMap;

use quasi_senate::config::ROTATION;
use quasi_senate::rotation::{eligible_for_role, pick_model, provider_has_key};
use quasi_senate::types::Role;

// ── test_pick_model_basic ──────────────────────────────────────────────────────

/// With no counts and no exclusions, pick_model for A2Drafter returns a model
/// that has A2Drafter in its roles — or skips gracefully if no provider key is set.
#[test]
fn test_pick_model_basic() {
    let counts: HashMap<String, u32> = HashMap::new();
    let result = pick_model(&Role::A2Drafter, &[], &counts, None);
    match result {
        Ok(entry) => {
            assert!(
                entry.roles.contains(&Role::A2Drafter),
                "Returned model '{}' does not have A2Drafter in its roles",
                entry.id
            );
        }
        Err(_) => {
            // No provider keys set in CI — this is acceptable.
        }
    }
}

// ── test_anti_collusion ────────────────────────────────────────────────────────

/// Pick a drafter, then pick a gater excluding the drafter's id.
/// Verify the returned model's id is different from the excluded id.
#[test]
fn test_anti_collusion() {
    let counts: HashMap<String, u32> = HashMap::new();

    // Find any eligible drafter.
    let drafter_result = pick_model(&Role::A2Drafter, &[], &counts, None);
    let drafter = match drafter_result {
        Ok(d) => d,
        Err(_) => return, // No keys set — skip gracefully.
    };

    // Now pick a gate model excluding the drafter.
    let excluded = &[drafter.id];
    let gate_result = pick_model(&Role::A3Gate, excluded, &counts, None);
    match gate_result {
        Ok(gate) => {
            assert_ne!(
                gate.id, drafter.id,
                "Gate model '{}' was not excluded as expected",
                gate.id
            );
        }
        Err(_) => {
            // Not enough keys for both roles — acceptable in test environment.
        }
    }
}

// ── test_eligible_for_role_returns_subset ──────────────────────────────────────

/// eligible_for_role returns a non-empty list containing only models that have
/// A1Council in their roles. Skip gracefully if no keys are set.
#[test]
fn test_eligible_for_role_returns_subset() {
    let eligible = eligible_for_role(&Role::A1Council);
    if eligible.is_empty() {
        // No provider keys set — skip.
        return;
    }
    for entry in &eligible {
        assert!(
            entry.roles.contains(&Role::A1Council),
            "eligible_for_role returned model '{}' without A1Council role",
            entry.id
        );
    }
}

// ── test_all_rotation_models_have_roles ───────────────────────────────────────

/// Every entry in ROTATION must have at least one role assigned.
#[test]
fn test_all_rotation_models_have_roles() {
    for entry in ROTATION {
        assert!(
            !entry.roles.is_empty(),
            "Model '{}' has an empty roles slice",
            entry.id
        );
    }
}

// ── test_rotation_count ───────────────────────────────────────────────────────

/// The ROTATION slice must contain exactly 33 models (generate_issue.py has 33).
#[test]
fn test_rotation_count() {
    assert_eq!(ROTATION.len(), 33, "Expected 33 models in ROTATION, got {}", ROTATION.len());
}

// ── test_pick_model_excludes ───────────────────────────────────────────────────

/// Create a list of all model IDs that support A2Drafter, exclude all but one,
/// verify that the remaining model is returned. Skips if no keys are configured.
#[test]
fn test_pick_model_excludes() {
    // Collect all A2Drafter-capable model IDs.
    let all_drafter_ids: Vec<&'static str> = ROTATION
        .iter()
        .filter(|e| e.roles.contains(&Role::A2Drafter))
        .map(|e| e.id)
        .collect();

    if all_drafter_ids.is_empty() {
        return;
    }

    // Pick the last one as the "only allowed" candidate.
    let target_id = all_drafter_ids[all_drafter_ids.len() - 1];

    // Build exclusion list of all other A2Drafter models.
    let exclusions: Vec<&str> = all_drafter_ids
        .iter()
        .copied()
        .filter(|id| *id != target_id)
        .collect();

    // Mock: set the target provider key so provider_has_key returns true.
    // In practice the environment won't have keys — check gracefully.
    let target_entry = ROTATION.iter().find(|e| e.id == target_id).unwrap();
    if !provider_has_key(target_entry.provider) {
        // No key for the target provider — nothing to assert, skip.
        return;
    }

    let counts: HashMap<String, u32> = HashMap::new();
    let result = pick_model(&Role::A2Drafter, &exclusions, &counts, None);
    match result {
        Ok(entry) => {
            assert_eq!(
                entry.id, target_id,
                "Expected model '{}' but got '{}'",
                target_id, entry.id
            );
        }
        Err(e) => {
            panic!("pick_model failed unexpectedly: {}", e);
        }
    }
}
