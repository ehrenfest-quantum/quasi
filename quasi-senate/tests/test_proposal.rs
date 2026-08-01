// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
// Tests for the quasi-board proposal path (A-track → quasi:Propose).
//
// The HTTP step is injected via `submit_proposal_with`, so these tests run
// without network access. The stubbed poster is the ONLY side effect in the
// proposal path — no outcome below involves GitHub issue creation.

use std::collections::HashSet;

use quasi_senate::ledger::{
    build_proposal_body, classify_response, submit_proposal_with, InboxResponse, ProposalOutcome,
};
use quasi_senate::pipeline::direct_issues_enabled;
use quasi_senate::types::IssueDraft;

fn draft() -> IssueDraft {
    IssueDraft {
        title: "Implement ZX-IR validation for boundary spiders".to_string(),
        description: "Add structural validation of boundary spiders.".to_string(),
        acceptance_criteria: vec![
            "cargo test -p afana passes".to_string(),
            "invalid graphs rejected".to_string(),
        ],
        label: "compiler".to_string(),
        drafter_model: "deepseek-v3".to_string(),
        phase_id: "PHASE-42".to_string(),
        estimated_effort: "medium".to_string(),
        affected_components: vec!["afana".to_string()],
    }
}

// ── Proposal body shape ───────────────────────────────────────────────────────

/// The body must carry exactly the keys `_process_activity` reads on the
/// board, with the correct `quasi:`-prefixed names — nothing more, nothing less.
#[test]
fn test_build_proposal_body_exact_key_set() {
    let body = build_proposal_body(&draft(), "gate reasoning");

    let top: HashSet<&str> = body
        .as_object()
        .expect("proposal body must be an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        HashSet::from(["@context", "type", "actor", "object"]),
        "top-level key set must be exact"
    );

    let object = body["object"].as_object().expect("object must be an object");
    let obj_keys: HashSet<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        obj_keys,
        HashSet::from([
            "quasi:title",
            "quasi:description",
            "quasi:estimatedEffort",
            "quasi:affectedComponents",
            "quasi:successCriteria",
            "quasi:level",
            "quasi:rationale",
        ]),
        "object key set must be exact"
    );

    assert_eq!(
        body["@context"].as_str().unwrap(),
        "https://www.w3.org/ns/activitystreams"
    );
    assert_eq!(body["type"].as_str().unwrap(), "quasi:Propose");
    assert_eq!(body["object"]["quasi:level"].as_str().unwrap(), "L1");
    assert_eq!(
        body["object"]["quasi:rationale"].as_str().unwrap(),
        "gate reasoning"
    );
}

/// Pin the easy-to-transpose mappings: affected_components →
/// quasi:affectedComponents, acceptance_criteria → quasi:successCriteria.
#[test]
fn test_build_proposal_body_field_mapping_not_transposed() {
    let body = build_proposal_body(&draft(), "r");
    let object = &body["object"];

    let affected: Vec<&str> = object["quasi:affectedComponents"]
        .as_array()
        .expect("quasi:affectedComponents must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(affected, vec!["afana"]);

    let criteria: Vec<&str> = object["quasi:successCriteria"]
        .as_array()
        .expect("quasi:successCriteria must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        criteria,
        vec!["cargo test -p afana passes", "invalid graphs rejected"]
    );

    assert_eq!(
        object["quasi:title"].as_str().unwrap(),
        "Implement ZX-IR validation for boundary spiders"
    );
    assert_eq!(object["quasi:estimatedEffort"].as_str().unwrap(), "medium");
}

// ── Response classification ───────────────────────────────────────────────────

#[test]
fn test_classify_202_queued_with_proposal_id() {
    let outcome = classify_response(202, r#"{"status": "proposed", "id": "prop-042"}"#);
    assert_eq!(outcome, ProposalOutcome::Queued("prop-042".to_string()));
}

#[test]
fn test_classify_400_rejected_verbatim_detail() {
    let outcome = classify_response(
        400,
        r#"{"detail": "trivial-effort proposals are not accepted — minimum scope is 'small'"}"#,
    );
    assert_eq!(
        outcome,
        ProposalOutcome::Rejected(
            "trivial-effort proposals are not accepted — minimum scope is 'small'".to_string()
        )
    );
}

#[test]
fn test_classify_409_duplicate_names_existing_proposal() {
    let outcome = classify_response(
        409,
        r#"{"detail": "Near-duplicate proposal detected (similarity 75%) — see existing proposal prop-007: 'ZX-IR validation'"}"#,
    );
    match outcome {
        ProposalOutcome::Duplicate(msg) => {
            assert!(msg.contains("prop-007"), "message must name the duplicate: {msg}");
        }
        other => panic!("expected Duplicate, got {other:?}"),
    }
}

#[test]
fn test_classify_429_l0_cap() {
    let outcome = classify_response(
        429,
        r#"{"detail": "L0 task cap reached: maximum 2 open L0 proposals at a time"}"#,
    );
    match outcome {
        ProposalOutcome::L0Cap(msg) => assert!(msg.contains("L0 task cap reached")),
        other => panic!("expected L0Cap, got {other:?}"),
    }
}

#[test]
fn test_classify_unexpected_status_is_failed() {
    let outcome = classify_response(500, "internal server error");
    match outcome {
        ProposalOutcome::Failed(msg) => assert!(msg.contains("500")),
        other => panic!("expected Failed, got {other:?}"),
    }
}

// ── Submit with injected HTTP poster ─────────────────────────────────────────

#[tokio::test]
async fn test_submit_202_reports_success() {
    let outcome = submit_proposal_with(
        |_body| async { Ok(InboxResponse { status: 202, body: r#"{"status":"proposed","id":"prop-099"}"#.to_string() }) },
        build_proposal_body(&draft(), "r"),
    )
    .await;
    assert_eq!(outcome, ProposalOutcome::Queued("prop-099".to_string()));
}

#[tokio::test]
async fn test_submit_400_rejected_no_retry() {
    let outcome = submit_proposal_with(
        |_body| async {
            Ok(InboxResponse {
                status: 400,
                body: r#"{"detail":"trivial-effort proposals are not accepted"}"#.to_string(),
            })
        },
        build_proposal_body(&draft(), "r"),
    )
    .await;
    assert!(matches!(outcome, ProposalOutcome::Rejected(_)));
}

#[tokio::test]
async fn test_submit_409_duplicate() {
    let outcome = submit_proposal_with(
        |_body| async {
            Ok(InboxResponse {
                status: 409,
                body: r#"{"detail":"Near-duplicate of prop-003"}"#.to_string(),
            })
        },
        build_proposal_body(&draft(), "r"),
    )
    .await;
    assert!(matches!(outcome, ProposalOutcome::Duplicate(_)));
}

#[tokio::test]
async fn test_submit_429_l0_cap() {
    let outcome = submit_proposal_with(
        |_body| async {
            Ok(InboxResponse { status: 429, body: r#"{"detail":"cap reached"}"#.to_string() })
        },
        build_proposal_body(&draft(), "r"),
    )
    .await;
    assert!(matches!(outcome, ProposalOutcome::L0Cap(_)));
}

#[tokio::test]
async fn test_submit_network_error_is_failed() {
    let outcome = submit_proposal_with(
        |_body| async { Err("connection refused".to_string()) },
        build_proposal_body(&draft(), "r"),
    )
    .await;
    match outcome {
        ProposalOutcome::Failed(msg) => assert!(msg.contains("connection refused")),
        other => panic!("expected Failed, got {other:?}"),
    }
}

// ── Rollback lever ────────────────────────────────────────────────────────────

#[test]
fn test_direct_issues_env_lever() {
    // Unset → proposal path (default).
    std::env::remove_var("SENATE_DIRECT_ISSUES");
    assert!(!direct_issues_enabled());

    // "1" → direct-issue path restored.
    std::env::set_var("SENATE_DIRECT_ISSUES", "1");
    assert!(direct_issues_enabled());

    // Any other value → proposal path.
    std::env::set_var("SENATE_DIRECT_ISSUES", "true");
    assert!(!direct_issues_enabled());

    std::env::remove_var("SENATE_DIRECT_ISSUES");
}
