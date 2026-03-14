// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
// Test prompt construction (no LLM calls)

use quasi_senate::prompts::{
    council_system_prompt, council_user_prompt, drafter_system_prompt, drafter_user_prompt,
    gate_system_prompt, gate_user_prompt, reviewer_system_prompt, solver_system_prompt,
    solver_user_prompt,
};
use quasi_senate::types::{Charter, IssueDraft, Quota};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dummy_charter() -> Charter {
    Charter {
        phase_id: "PHASE-TEST".to_string(),
        date: "2026-03-01".to_string(),
        frontier_level: 2,
        goal: "Test goal".to_string(),
        priorities: vec![],
        blocked_topics: vec!["CI".to_string()],
        quota: Quota {
            total_issues_this_phase: 10,
            max_per_priority: 3,
            max_l0_issues: 2,
        },
        notes_to_reviewers: "Review carefully".to_string(),
    }
}

fn dummy_draft() -> IssueDraft {
    IssueDraft {
        title: "Implement ZX-IR gate fusion pass".to_string(),
        description: "Add a gate fusion pass to the ZX-IR pipeline.".to_string(),
        acceptance_criteria: vec!["Tests pass".to_string()],
        label: "compiler".to_string(),
        drafter_model: "deepseek-v3".to_string(),
        phase_id: "PHASE-TEST".to_string(),
    }
}

// ── System prompt tests ───────────────────────────────────────────────────────

#[test]
fn test_council_system_prompt_nonempty() {
    assert!(!council_system_prompt().is_empty(), "council_system_prompt() returned empty string");
}

#[test]
fn test_council_system_prompt_contains_mvp() {
    let prompt = council_system_prompt();
    assert!(
        prompt.contains("Ehrenfest"),
        "council_system_prompt() does not mention 'Ehrenfest'"
    );
    assert!(
        prompt.contains("QPU"),
        "council_system_prompt() does not mention 'QPU'"
    );
}

#[test]
fn test_drafter_system_prompt_contains_json() {
    let prompt = drafter_system_prompt();
    assert!(
        prompt.contains("JSON"),
        "drafter_system_prompt() does not mention 'JSON'"
    );
}

#[test]
fn test_gate_system_prompt_contains_reject() {
    let prompt = gate_system_prompt();
    assert!(
        prompt.contains("reject"),
        "gate_system_prompt() does not mention 'reject'"
    );
}

#[test]
fn test_solver_system_prompt_contains_edits() {
    let prompt = solver_system_prompt();
    assert!(
        prompt.contains("edits"),
        "solver_system_prompt() does not mention 'edits'"
    );
}

#[test]
fn test_reviewer_system_prompt_contains_approve() {
    let prompt = reviewer_system_prompt();
    assert!(
        prompt.contains("approve"),
        "reviewer_system_prompt() does not mention 'approve'"
    );
}

// ── User prompt tests ─────────────────────────────────────────────────────────

#[test]
fn test_council_user_prompt_contains_context() {
    let architecture = "QUASI_ARCH_STRING_UNIQUE_SENTINEL";
    let result = council_user_prompt(
        architecture,
        "## Roadmap\nSome roadmap",
        "## Open Issues\nNone",
        "## Merged PRs\nNone",
        None,
        "## Leaderboard\nNone",
    );
    assert!(
        result.contains(architecture),
        "council_user_prompt() does not contain the architecture string"
    );
}

#[test]
fn test_drafter_user_prompt_contains_charter() {
    let charter = dummy_charter();
    let result = drafter_user_prompt(&charter, "src/", "abc123", "No open issues", 2);
    // The charter is serialized as JSON in the prompt, so phase_id should appear.
    assert!(
        result.contains("PHASE-TEST"),
        "drafter_user_prompt() does not contain the charter's phase_id 'PHASE-TEST'"
    );
}

#[test]
fn test_gate_user_prompt_contains_draft_title() {
    let charter = dummy_charter();
    let draft = dummy_draft();
    let result = gate_user_prompt(&charter, &draft, "No open issues");
    assert!(
        result.contains("Implement ZX-IR gate fusion pass"),
        "gate_user_prompt() does not contain the draft title"
    );
}

#[test]
fn test_solver_system_prompt_warns_against_duplicate_modules() {
    let prompt = solver_system_prompt();
    assert!(
        prompt.contains("Check existing implementation"),
        "solver prompt must warn against duplicate modules"
    );
    assert!(
        prompt.contains("Do not create parallel/duplicate modules"),
        "solver prompt must forbid parallel modules"
    );
    assert!(
        prompt.contains("MUST compile"),
        "solver prompt must require compilation"
    );
}

#[test]
fn test_reviewer_system_prompt_rejects_stubs() {
    let prompt = reviewer_system_prompt();
    assert!(
        prompt.contains("stub/placeholder"),
        "reviewer prompt must reject stubs"
    );
    assert!(
        prompt.contains("Duplicate module"),
        "reviewer prompt must reject duplicate modules"
    );
}

#[test]
fn test_gate_system_prompt_rejects_trivial_gate_issues() {
    let prompt = gate_system_prompt();
    assert!(
        prompt.contains("gate already"),
        "gate prompt must reject redundant gate issues"
    );
}

#[test]
fn test_drafter_system_prompt_has_anti_patterns() {
    let prompt = drafter_system_prompt();
    assert!(
        prompt.contains("Anti-patterns"),
        "drafter prompt must list anti-patterns"
    );
}

#[test]
fn test_solver_user_prompt_contains_issue_title() {
    let issue_title = "Add ZX-IR rewrite rules for CX decomposition";
    let result = solver_user_prompt(issue_title, "## Body\nSome body", "## Context\nNone");
    assert!(
        result.contains(issue_title),
        "solver_user_prompt() does not contain the issue title"
    );
}
