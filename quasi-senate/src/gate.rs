// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! A.3 Gate review role.

use anyhow::Result;
use std::collections::HashMap;

use crate::types::{Charter, GateVerdict, IssueDraft, Role, RotationEntry, Verdict};

/// Private raw struct for LLM JSON response.
#[derive(serde::Deserialize)]
struct GateVerdictRaw {
    verdict: String, // "approve" or "reject"
    reasoning: String,
    suggestions: Option<String>,
}

/// Review a draft issue (A.3). Returns verdict and the rotation entry used.
///
/// * `exclude` — model IDs to exclude (at minimum: the drafter's model id)
pub async fn gate_review(
    charter: &Charter,
    draft: &IssueDraft,
    open_issues: &str,
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
    dry_run: bool,
) -> Result<(GateVerdict, &'static RotationEntry, crate::provider::CallResult)> {
    // 1. Pick model
    let entry = crate::rotation::pick_model(&Role::A3Gate, exclude, counts, last_provider)?;

    // 2. Build prompts
    let system = crate::prompts::gate_system_prompt();
    let user = crate::prompts::gate_user_prompt(charter, draft, open_issues);

    // 3. Dry-run path — return an approving verdict
    if dry_run {
        println!(
            "[dry-run] gate: would call model '{}' to review draft {:?}",
            entry.id, draft.title,
        );
        let verdict = GateVerdict {
            verdict: Verdict::Approve,
            reasoning: "dry-run auto-approve".to_string(),
            suggestions: None,
            reviewer_model: entry.id.to_string(),
        };
        let dummy_call = crate::provider::CallResult {
            content: "dry-run".to_string(),
            latency_ms: 0,
            http_status: 0,
            retries: 0,
            model_verified: None,
            served_model: None,
        };
        return Ok((verdict, entry, dummy_call));
    }

    // 4. Call the LLM
    let call_result = crate::provider::call_model(entry, &system, &user, 0.2, 1024).await?;
    let raw = call_result.content.clone();

    // 5. Parse raw response
    let raw_verdict = crate::provider::parse_json_response::<GateVerdictRaw>(&raw)?;

    // 6. Map verdict string
    let verdict_enum = match raw_verdict.verdict.to_lowercase().as_str() {
        "approve" => Verdict::Approve,
        _ => Verdict::Reject,
    };

    let gate_verdict = GateVerdict {
        verdict: verdict_enum,
        reasoning: raw_verdict.reasoning,
        suggestions: raw_verdict.suggestions,
        reviewer_model: entry.id.to_string(),
    };

    tracing::info!(
        "Gate review complete: model={} verdict={} draft={:?}",
        entry.id,
        gate_verdict.verdict,
        draft.title,
    );

    // 7. Return
    Ok((gate_verdict, entry, call_result))
}
