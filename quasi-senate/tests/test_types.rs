// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
// Serde round-trip tests for all types

use std::collections::HashMap;

use quasi_senate::types::{
    Charter, FileEdit, GateVerdict, IssueDraft, Priority, Quota, ReviewVerdict, Role,
    SenateState, SolveResult, Verdict,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dummy_charter() -> Charter {
    Charter {
        phase_id: "PHASE-42".to_string(),
        date: "2026-03-01".to_string(),
        frontier_level: 2,
        goal: "Advance ZX-IR compilation".to_string(),
        priorities: vec![Priority {
            rank: 1,
            area: "Afana compiler".to_string(),
            description: "Gate fusion pass".to_string(),
            max_issues: 4,
            level: 2,
        }],
        blocked_topics: vec!["CI".to_string()],
        quota: Quota {
            total_issues_this_phase: 15,
            max_per_priority: 4,
            max_l0_issues: 3,
        },
        notes_to_reviewers: "Reject docs-only issues".to_string(),
    }
}

fn dummy_issue_draft() -> IssueDraft {
    IssueDraft {
        title: "Implement CNOT decomposition in ZX-IR".to_string(),
        description: "Add CNOT → ZX spider decomposition.".to_string(),
        acceptance_criteria: vec!["Tests pass".to_string(), "QASM3 output valid".to_string()],
        label: "compiler".to_string(),
        drafter_model: "deepseek-v3".to_string(),
        phase_id: "PHASE-42".to_string(),
    }
}

fn dummy_solve_result() -> SolveResult {
    SolveResult {
        reasoning: "Inserted CNOT decomposition pass.".to_string(),
        edits: vec![FileEdit {
            file: "afana/zx_ir.py".to_string(),
            find: "def old_fn():".to_string(),
            replace: "def new_fn():".to_string(),
        }],
        new_files: HashMap::new(),
        solver_model: "deepseek-v3".to_string(),
    }
}

// ── Round-trip tests ──────────────────────────────────────────────────────────

#[test]
fn test_charter_round_trip() {
    let original = dummy_charter();
    let json = serde_json::to_string(&original).expect("Charter serialization failed");
    let restored: Charter = serde_json::from_str(&json).expect("Charter deserialization failed");
    assert_eq!(restored.phase_id, original.phase_id);
    assert_eq!(restored.frontier_level, original.frontier_level);
    assert_eq!(restored.quota.total_issues_this_phase, original.quota.total_issues_this_phase);
}

#[test]
fn test_issue_draft_round_trip() {
    let original = dummy_issue_draft();
    let json = serde_json::to_string(&original).expect("IssueDraft serialization failed");
    let restored: IssueDraft =
        serde_json::from_str(&json).expect("IssueDraft deserialization failed");
    assert_eq!(restored.title, original.title);
    assert_eq!(restored.phase_id, original.phase_id);
    assert_eq!(restored.acceptance_criteria, original.acceptance_criteria);
}

#[test]
fn test_gate_verdict_round_trip() {
    let original = GateVerdict {
        verdict: Verdict::Approve,
        reasoning: "Passes all criteria.".to_string(),
        suggestions: None,
        reviewer_model: "qwq-32b".to_string(),
    };
    let json = serde_json::to_string(&original).expect("GateVerdict serialization failed");
    let restored: GateVerdict =
        serde_json::from_str(&json).expect("GateVerdict deserialization failed");
    assert_eq!(restored.verdict, Verdict::Approve);
    assert_eq!(restored.reviewer_model, original.reviewer_model);
}

#[test]
fn test_gate_verdict_reject_round_trip() {
    let original = GateVerdict {
        verdict: Verdict::Reject,
        reasoning: "Duplicates an existing issue.".to_string(),
        suggestions: Some("Narrow the scope to gate fusion only.".to_string()),
        reviewer_model: "qwq-32b".to_string(),
    };
    let json = serde_json::to_string(&original).expect("GateVerdict(Reject) serialization failed");
    let restored: GateVerdict =
        serde_json::from_str(&json).expect("GateVerdict(Reject) deserialization failed");
    assert_eq!(restored.verdict, Verdict::Reject);
    assert!(restored.suggestions.is_some());
}

#[test]
fn test_solve_result_round_trip() {
    let original = dummy_solve_result();
    let json = serde_json::to_string(&original).expect("SolveResult serialization failed");
    let restored: SolveResult =
        serde_json::from_str(&json).expect("SolveResult deserialization failed");
    assert_eq!(restored.solver_model, original.solver_model);
    assert_eq!(restored.edits.len(), original.edits.len());
    assert_eq!(restored.edits[0].file, original.edits[0].file);
}

#[test]
fn test_review_verdict_round_trip() {
    let original = ReviewVerdict {
        verdict: Verdict::Approve,
        reasoning: "Solution satisfies acceptance criteria.".to_string(),
        issues: vec![],
        suggested_fix: None,
        reviewer_model: "gemma-3-27b".to_string(),
    };
    let json = serde_json::to_string(&original).expect("ReviewVerdict serialization failed");
    let restored: ReviewVerdict =
        serde_json::from_str(&json).expect("ReviewVerdict deserialization failed");
    assert_eq!(restored.verdict, Verdict::Approve);
    assert_eq!(restored.reviewer_model, original.reviewer_model);
    assert!(restored.issues.is_empty());
}

#[test]
fn test_senate_state_default() {
    let state = SenateState::default();
    assert!(state.current_charter.is_none(), "current_charter should default to None");
    assert!(state.charter_path.is_none(), "charter_path should default to None");
    assert!(state.last_generate_provider.is_none(), "last_generate_provider should default to None");
    assert!(state.last_solve_provider.is_none(), "last_solve_provider should default to None");
    assert!(state.draft_retries.is_empty(), "draft_retries should default to empty map");
    assert!(state.solve_retries.is_empty(), "solve_retries should default to empty map");
    assert!(state.phase_issue_count.is_empty(), "phase_issue_count should default to empty map");
}

#[test]
fn test_verdict_serialization() {
    let json = serde_json::to_string(&Verdict::Approve).expect("Verdict::Approve serialization failed");
    // serde rename_all = "snake_case" → "approve"
    assert_eq!(json, "\"approve\"", "Verdict::Approve should serialize to \"approve\"");
}

#[test]
fn test_verdict_reject_serialization() {
    let json = serde_json::to_string(&Verdict::Reject).expect("Verdict::Reject serialization failed");
    assert_eq!(json, "\"reject\"", "Verdict::Reject should serialize to \"reject\"");
}

#[test]
fn test_role_serialization() {
    let json = serde_json::to_string(&Role::A1Council).expect("Role::A1Council serialization failed");
    // serde rename_all = "snake_case" → "a1_council"
    assert_eq!(json, "\"a1_council\"", "Role::A1Council should serialize to \"a1_council\"");
}
