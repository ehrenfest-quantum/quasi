// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! B.2 Solution Reviewer role.

use anyhow::Result;
use std::collections::HashMap;

use crate::types::{ReviewVerdict, Role, RotationEntry, SolveResult, Verdict};

/// Private raw struct for LLM JSON response.
#[derive(serde::Deserialize)]
struct ReviewVerdictRaw {
    verdict: String, // "approve" or "request_changes"
    reasoning: String,
    #[serde(default)]
    issues: Vec<String>,
    suggested_fix: Option<String>,
}

/// Review a proposed solution (B.2). Returns verdict and the rotation entry used.
///
/// * `exclude` — model IDs to exclude (at minimum: drafter and solver model IDs)
pub async fn review_solution(
    issue_title: &str,
    issue_body: &str,
    solve_result: &SolveResult,
    repo_context: &str,
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
    dry_run: bool,
) -> Result<(ReviewVerdict, &'static RotationEntry, crate::provider::CallResult)> {
    // 1. Pick model
    let entry = crate::rotation::pick_model(&Role::B2Reviewer, exclude, counts, last_provider)?;

    // 2. Build prompts
    let system = crate::prompts::reviewer_system_prompt();
    let user = crate::prompts::reviewer_user_prompt(issue_title, issue_body, solve_result, repo_context);

    // 3. Dry-run path — return an approving verdict
    if dry_run {
        println!(
            "[dry-run] reviewer: would call model '{}' to review solution for {:?}",
            entry.id, issue_title,
        );
        let verdict = ReviewVerdict {
            verdict: Verdict::Approve,
            reasoning: "dry-run auto-approve".to_string(),
            issues: vec![],
            suggested_fix: None,
            reviewer_model: entry.id.to_string(),
        };
        let dummy_call = crate::provider::CallResult {
            content: "dry-run".to_string(),
            latency_ms: 0,
            http_status: 0,
            retries: 0,
            model_verified: None,
            served_model: None,
            input_len: 0,
        };
        return Ok((verdict, entry, dummy_call));
    }

    // 4. Call the LLM
    let call_result = crate::provider::call_model(entry, &system, &user, 0.2, 2048).await?;
    let raw = call_result.content.clone();

    // 5. Parse raw response — map failure to ParseFailure so pipeline can write telemetry.
    let raw_verdict = crate::provider::parse_json_response::<ReviewVerdictRaw>(&raw)
        .map_err(|e| crate::provider::ParseFailure {
            call: call_result.clone(),
            entry,
            error: e.to_string(),
        })?;

    // 6. Map verdict string
    let verdict_enum = match raw_verdict.verdict.to_lowercase().as_str() {
        "approve" => Verdict::Approve,
        _ => Verdict::RequestChanges,
    };

    let review_verdict = ReviewVerdict {
        verdict: verdict_enum,
        reasoning: raw_verdict.reasoning,
        issues: raw_verdict.issues,
        suggested_fix: raw_verdict.suggested_fix,
        reviewer_model: entry.id.to_string(),
    };

    tracing::info!(
        "Reviewer complete: model={} verdict={} issue={:?}",
        entry.id,
        review_verdict.verdict,
        issue_title,
    );

    // 7. Return
    Ok((review_verdict, entry, call_result))
}
