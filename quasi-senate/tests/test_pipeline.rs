// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
// Integration tests with mock provider
//
// These tests verify pipeline logic without making real HTTP calls.
// They test parse_json_response and SenateState round-trip serialization.

use std::collections::HashMap;

use quasi_senate::provider::parse_json_response;
use quasi_senate::types::SenateState;

// ── parse_json_response tests ─────────────────────────────────────────────────

/// Clean JSON object parses without any repair needed.
#[test]
fn test_parse_json_response_clean() {
    let input = r#"{"key": "value"}"#;
    let result: serde_json::Value =
        parse_json_response(input).expect("Clean JSON should parse successfully");
    assert_eq!(
        result["key"].as_str().unwrap(),
        "value",
        "Parsed value for 'key' should be 'value'"
    );
}

/// JSON wrapped in ```json ... ``` markdown fences is stripped and parsed.
#[test]
fn test_parse_json_response_with_fences() {
    let input = "```json\n{\"key\": \"value\"}\n```";
    let result: serde_json::Value =
        parse_json_response(input).expect("Fenced JSON should parse successfully");
    assert_eq!(
        result["key"].as_str().unwrap(),
        "value",
        "Parsed value for 'key' should be 'value' after stripping fences"
    );
}

/// JSON with a leading JS-style `//` comment is repaired and parsed.
#[test]
fn test_parse_json_response_with_comments() {
    let input = "// comment\n{\"key\": \"value\"}";
    let result: serde_json::Value =
        parse_json_response(input).expect("JSON with comment should parse successfully");
    assert_eq!(
        result["key"].as_str().unwrap(),
        "value",
        "Parsed value for 'key' should be 'value' after stripping comment"
    );
}

/// JSON where a string value contains a literal newline character is repaired via
/// fix_literal_newlines and then parsed successfully.
#[test]
fn test_parse_json_response_with_literal_newlines() {
    // The JSON spec forbids raw U+000A inside a string literal, but some LLMs emit it.
    // parse_json_response should escape it automatically.
    let input = "{\"key\": \"line1\nline2\"}";
    let result: serde_json::Value = parse_json_response(input)
        .expect("JSON with literal newline in string value should be repaired and parsed");
    let key_val = result["key"].as_str().unwrap();
    assert!(
        key_val.contains("line1"),
        "Repaired value should still contain 'line1'"
    );
    assert!(
        key_val.contains("line2"),
        "Repaired value should still contain 'line2'"
    );
}

// ── SenateState serialization round-trip ─────────────────────────────────────

/// SenateState with populated fields survives a JSON round-trip intact.
#[test]
fn test_senate_state_serialization() {
    let mut draft_retries = HashMap::new();
    draft_retries.insert("issue-123".to_string(), 2u8);

    let mut phase_issue_count = HashMap::new();
    phase_issue_count.insert("PHASE-42".to_string(), 7u16);

    let original = SenateState {
        current_charter: Some("PHASE-42".to_string()),
        charter_path: Some("/home/vops/state/charter.json".to_string()),
        last_generate_provider: Some("openrouter".to_string()),
        last_solve_provider: Some("groq".to_string()),
        draft_retries,
        solve_retries: HashMap::new(),
        phase_issue_count,
    };

    let json =
        serde_json::to_string_pretty(&original).expect("SenateState serialization should succeed");

    let restored: SenateState =
        serde_json::from_str(&json).expect("SenateState deserialization should succeed");

    assert_eq!(
        restored.current_charter,
        Some("PHASE-42".to_string()),
        "current_charter should survive round-trip"
    );
    assert_eq!(
        restored.charter_path,
        Some("/home/vops/state/charter.json".to_string()),
        "charter_path should survive round-trip"
    );
    assert_eq!(
        restored.last_generate_provider,
        Some("openrouter".to_string()),
        "last_generate_provider should survive round-trip"
    );
    assert_eq!(
        restored.last_solve_provider,
        Some("groq".to_string()),
        "last_solve_provider should survive round-trip"
    );
    assert_eq!(
        restored.draft_retries.get("issue-123").copied(),
        Some(2u8),
        "draft_retries['issue-123'] should be 2 after round-trip"
    );
    assert_eq!(
        restored.phase_issue_count.get("PHASE-42").copied(),
        Some(7u16),
        "phase_issue_count['PHASE-42'] should be 7 after round-trip"
    );
    assert!(
        restored.solve_retries.is_empty(),
        "solve_retries should still be empty after round-trip"
    );
}
