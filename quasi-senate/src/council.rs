// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! A.1 Architecture Council session.

use anyhow::Result;
use std::collections::HashMap;

use crate::github::GitHubClient;
use crate::provider::CallResult;
use crate::types::{Charter, Role, RotationEntry};

/// Run one Architecture Council session (A.1).
/// Returns the emitted Charter together with the rotation entry and call metadata
/// so the pipeline can write telemetry.
pub async fn run_council(
    github: &GitHubClient,
    dry_run: bool,
) -> Result<(Charter, &'static RotationEntry, CallResult)> {
    // 1. Fetch ARCHITECTURE.md
    let arch_content = github
        .get_file("ARCHITECTURE.md", "main")
        .await
        .map(|fc| fc.content)
        .unwrap_or_else(|_| "(ARCHITECTURE.md unavailable)".to_string());

    // 2. Fetch ROADMAP.md
    let roadmap_content = github
        .get_file("ROADMAP.md", "main")
        .await
        .map(|fc| fc.content)
        .unwrap_or_else(|_| "(ROADMAP.md unavailable)".to_string());

    // 3. Fetch open issues list
    let open_issues = github.list_open_issues(40).await.unwrap_or_default();
    let open_issues_str: String = open_issues
        .iter()
        .map(|i| format!("  #{}: {}\n", i.number, i.title))
        .collect();

    // 4. Fetch merged PRs since 30 days ago
    let thirty_days_ago = chrono_minus_30_days();
    let merged_prs = github
        .list_merged_prs_since(&thirty_days_ago)
        .await
        .unwrap_or_default();
    let merged_prs_str: String = merged_prs
        .iter()
        .map(|pr| format!("  #{}: {}\n", pr.number, pr.title))
        .collect();

    // 5. Previous charter: attempt to read from state file path
    let previous_charter: Option<String> = None; // loaded by caller if needed

    // 6. Leaderboard placeholder
    let leaderboard = "(Pauli-Test leaderboard not yet fetched)";

    // 7. Pick a model via rotation
    let counts: HashMap<String, u32> = HashMap::new();
    let entry = crate::rotation::pick_model(&Role::A1Council, &[], &counts, None)?;

    // 8. Build prompts
    let system = crate::prompts::council_system_prompt();
    let user = crate::prompts::council_user_prompt(
        &arch_content,
        &roadmap_content,
        &open_issues_str,
        &merged_prs_str,
        previous_charter.as_deref(),
        leaderboard,
    );

    // 9. Dry-run path
    if dry_run {
        println!(
            "[dry-run] council: would call model '{}' with {} chars system + {} chars user",
            entry.id,
            system.len(),
            user.len(),
        );
        let dummy_charter = Charter {
            phase_id: "dry-run-phase".to_string(),
            date: "2026-01-01".to_string(),
            frontier_level: 1,
            goal: "Dry-run placeholder goal".to_string(),
            priorities: vec![],
            blocked_topics: vec![],
            quota: crate::types::Quota {
                total_issues_this_phase: 0,
                max_per_priority: 0,
                max_l0_issues: 0,
            },
            notes_to_reviewers: "dry-run".to_string(),
        };
        let dummy_call = CallResult {
            content: "dry-run".to_string(),
            latency_ms: 0,
            http_status: 0,
            retries: 0,
            model_verified: None,
            served_model: None,
            input_len: 0,
        };
        return Ok((dummy_charter, entry, dummy_call));
    }

    // 10. Call the LLM
    let call_result = crate::provider::call_model(entry, system, &user, 0.2, 8192).await?;

    // 11. Parse response
    let charter =
        crate::provider::parse_json_response::<Charter>(&call_result.content)
            .map_err(|e| crate::provider::ParseFailure {
                call: call_result.clone(),
                entry,
                error: e.to_string(),
            })?;

    // 12. Log
    tracing::info!("Council session complete: phase_id={}", charter.phase_id);

    // 13. Return
    Ok((charter, entry, call_result))
}

/// Return an ISO 8601 date string 30 days before today.
///
/// Uses only stdlib — avoids the `chrono` / `time` crate dependency.
/// Accuracy: correct for all months except it treats every month as having
/// a fixed number of days counted backwards, which is exact for 30-day
/// look-backs because we just subtract 30 from the Julian day number.
fn chrono_minus_30_days() -> String {
    // We embed a minimal Julian-day computation to stay stdlib-only.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days since Unix epoch
    let days_since_epoch = (secs / 86400) as i64;
    // Julian day number for 1970-01-01 is 2440588
    let jdn = days_since_epoch + 2440588 - 30;
    // Convert JDN back to calendar date (algorithm from Wikipedia)
    let l = jdn + 68569;
    let n = (4 * l) / 146097;
    let l = l - (146097 * n + 3) / 4;
    let i = (4000 * (l + 1)) / 1461001;
    let l = l - (1461 * i) / 4 + 31;
    let j = (80 * l) / 2447;
    let day = l - (2447 * j) / 80;
    let l = j / 11;
    let month = j + 2 - 12 * l;
    let year = 100 * (n - 49) + i + l;
    format!("{:04}-{:02}-{:02}", year, month, day)
}
