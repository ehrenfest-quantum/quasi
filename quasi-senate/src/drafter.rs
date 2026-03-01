// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! A.2 Issue Drafter role.

use anyhow::Result;
use std::collections::HashMap;

use crate::github::GitHubClient;
use crate::types::{Charter, IssueDraft, Role, RotationEntry};

/// Private raw struct for LLM JSON response.
#[derive(serde::Deserialize)]
struct IssueDraftRaw {
    title: String,
    description: String,
    acceptance_criteria: Vec<String>,
    label: String,
}

/// Draft one issue (A.2). Returns the draft and the rotation entry used.
///
/// * `drafter_exclude` — model IDs to exclude (anti-collusion from previous attempt)
/// * `retry_feedback` — if this is a retry, include rejection feedback
pub async fn draft_issue(
    github: &GitHubClient,
    charter_json: &str,
    level: u8,
    drafter_exclude: &[&str],
    retry_feedback: Option<&str>,
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
    dry_run: bool,
) -> Result<(IssueDraft, &'static RotationEntry, crate::provider::CallResult)> {
    // 1. Pick model
    let entry = crate::rotation::pick_model(&Role::A2Drafter, drafter_exclude, counts, last_provider)?;

    // 2. Fetch open issues for dedup context
    let open_issues = github.list_open_issues(40).await.unwrap_or_default();
    let open_issues_str: String = open_issues
        .iter()
        .map(|i| format!("  #{}: {}\n", i.number, i.title))
        .collect();

    // 3. Fetch ARCHITECTURE.md for file tree context
    let file_tree = github
        .get_file("ARCHITECTURE.md", "main")
        .await
        .map(|fc| fc.content)
        .unwrap_or_else(|_| "(ARCHITECTURE.md unavailable)".to_string());

    // 4. Recent commits placeholder (unavailable in daemon mode)
    let recent_commits = "(recent commits unavailable in daemon mode)";

    // 5. Parse charter from JSON
    let charter: Charter = serde_json::from_str(charter_json)?;

    // 6. Build user prompt
    let mut user = crate::prompts::drafter_user_prompt(
        &charter,
        &file_tree,
        recent_commits,
        &open_issues_str,
        level,
    );

    // 7. Append retry feedback if present
    if let Some(feedback) = retry_feedback {
        user.push_str(&format!(
            "\n\n## Previous rejection feedback\n{feedback}\n\nPlease revise your proposal to address this feedback."
        ));
    }

    // 8. Dry-run path
    if dry_run {
        println!(
            "[dry-run] drafter: would call model '{}' for level={} charter={}",
            entry.id, level, charter.phase_id,
        );
        let placeholder = IssueDraft {
            title: "dry-run: placeholder issue".to_string(),
            description: "Dry-run placeholder description.".to_string(),
            acceptance_criteria: vec!["Dry-run criterion.".to_string()],
            label: "compiler".to_string(),
            drafter_model: entry.id.to_string(),
            phase_id: charter.phase_id.clone(),
        };
        let dummy_call = crate::provider::CallResult {
            content: "dry-run".to_string(),
            latency_ms: 0,
            http_status: 0,
            retries: 0,
            model_verified: None,
            served_model: None,
        };
        return Ok((placeholder, entry, dummy_call));
    }

    // 9. Call the LLM
    let system = crate::prompts::drafter_system_prompt();
    let call_result = crate::provider::call_model(entry, &system, &user, 0.7, 2048).await?;
    let raw = call_result.content.clone();

    // 10. Parse raw response
    let raw_draft = crate::provider::parse_json_response::<IssueDraftRaw>(&raw)?;

    // 11. Convert to IssueDraft
    let draft = IssueDraft {
        title: raw_draft.title,
        description: raw_draft.description,
        acceptance_criteria: raw_draft.acceptance_criteria,
        label: raw_draft.label,
        drafter_model: entry.id.to_string(),
        phase_id: charter.phase_id.clone(),
    };

    tracing::info!(
        "Drafter complete: model={} title={:?} phase={}",
        entry.id,
        draft.title,
        draft.phase_id,
    );

    // 12. Return
    Ok((draft, entry, call_result))
}
