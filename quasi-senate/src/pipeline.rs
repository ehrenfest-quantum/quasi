// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Orchestrates the full A-track and B-track Senate Loop pipelines.
//!
//! This is the heart of the daemon — `run_council`, `run_draft_pipeline`,
//! `run_solve_pipeline`, `run_cycle`, and `run_batch` are all wired here.

use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::fedi::FediClient;
use crate::github::GitHubClient;
use crate::matrix::MatrixBot;
use crate::state::save_state;
use crate::types::{Charter, SenateState, Verdict};

/// Shared context passed through every pipeline stage.
pub struct AppContext {
    pub github: GitHubClient,
    pub matrix: Option<MatrixBot>,
    pub fedi: Option<FediClient>,
    pub state: SenateState,
    pub dry_run: bool,
}

// ── A.1 — Architecture Council ────────────────────────────────────────────────

/// Run the A.1 Architecture Council session.
///
/// Emits a new charter, persists it to state, and posts to Matrix + Fedi.
pub async fn run_council(ctx: &mut AppContext) -> Result<Charter> {
    info!("pipeline: running A.1 Architecture Council session");

    // 1. Call the council module
    let charter = crate::council::run_council(&ctx.github, ctx.dry_run).await?;

    // 2. Save charter to state
    let charter_json = serde_json::to_string(&charter)?;
    ctx.state.current_charter = Some(charter_json.clone());
    save_state(&ctx.state)?;

    // 3. Post charter to Matrix #senate-council
    if let Some(bot) = &ctx.matrix {
        // Identify which model was used — not tracked directly, use "council"
        let (plain, html) = crate::matrix::format_charter_message(&charter, "council");
        match bot.join_room("#senate-council:matrix.arvak.io").await {
            Ok(room_id) => {
                if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                    warn!("pipeline: failed to post charter to Matrix: {err}");
                }
            }
            Err(err) => {
                warn!("pipeline: failed to join #senate-council: {err}");
            }
        }
    }

    // 4. Post to Fedi
    if let Some(fedi) = &ctx.fedi {
        let status = format!(
            "🏛️ QUASI Architecture Council — Phase {} started. Goal: {}",
            charter.phase_id, charter.goal
        );
        if let Err(err) = fedi.post_status(&status, "unlisted").await {
            warn!("pipeline: fedi post_status failed: {err}");
        }
    }

    Ok(charter)
}

// ── A-track ───────────────────────────────────────────────────────────────────

/// Run the A-track pipeline for one issue: A.2 draft, A.3 gate, retry up to 2×.
///
/// On approval, opens the GitHub issue. Returns the issue number.
pub async fn run_draft_pipeline(ctx: &mut AppContext) -> Result<u32> {
    info!("pipeline: starting A-track draft pipeline");

    // 1. Load current charter
    let charter_json = ctx
        .state
        .current_charter
        .as_deref()
        .ok_or_else(|| anyhow!("No charter found. Run `senate council` first."))?
        .to_string();

    // 2. Parse charter
    let charter: Charter = serde_json::from_str(&charter_json)
        .map_err(|e| anyhow!("Failed to parse charter JSON: {e}"))?;

    // 3. List open issues for dedup context
    let open_issues_vec = ctx.github.list_open_issues(40).await.unwrap_or_default();
    let open_issues_str: String = open_issues_vec
        .iter()
        .map(|i| format!("  #{}: {}\n", i.number, i.title))
        .collect();

    // 4. Determine level from charter
    let level = charter.frontier_level;

    let counts: HashMap<String, u32> = HashMap::new();
    let last_provider = ctx.state.last_generate_provider.as_deref();

    let mut drafter_exclude: Vec<String> = Vec::new();
    let mut retry_feedback: Option<String> = None;

    // 5. Retry loop — up to 2 attempts
    for retry in 0..2u32 {
        info!("pipeline: A.2 draft attempt {}", retry + 1);

        let exclude_refs: Vec<&str> = drafter_exclude.iter().map(|s| s.as_str()).collect();

        // A.2 — Draft
        let (draft, a2_entry) = crate::drafter::draft_issue(
            &ctx.github,
            &charter_json,
            level,
            &exclude_refs,
            retry_feedback.as_deref(),
            &counts,
            last_provider,
            ctx.dry_run,
        )
        .await?;

        // Post draft to Matrix #senate-drafts
        if let Some(bot) = &ctx.matrix {
            let (plain, html) = crate::matrix::format_draft_message(&draft);
            match bot.join_room("#senate-drafts:matrix.arvak.io").await {
                Ok(room_id) => {
                    if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                        warn!("pipeline: failed to post draft to Matrix: {err}");
                    }
                }
                Err(err) => {
                    warn!("pipeline: failed to join #senate-drafts: {err}");
                }
            }
        }

        // A.3 — Gate review (exclude the drafter)
        let gate_exclude: Vec<&str> = {
            let mut v: Vec<&str> = exclude_refs.clone();
            v.push(a2_entry.id);
            v
        };

        let (verdict, _a3_entry) = crate::gate::gate_review(
            &charter,
            &draft,
            &open_issues_str,
            &gate_exclude,
            &counts,
            last_provider,
            ctx.dry_run,
        )
        .await?;

        // Post verdict to Matrix #senate-drafts
        if let Some(bot) = &ctx.matrix {
            let (plain, html) = crate::matrix::format_verdict_message(&verdict, &draft.title);
            match bot.join_room("#senate-drafts:matrix.arvak.io").await {
                Ok(room_id) => {
                    if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                        warn!("pipeline: failed to post gate verdict to Matrix: {err}");
                    }
                }
                Err(err) => {
                    warn!("pipeline: failed to join #senate-drafts: {err}");
                }
            }
        }

        if verdict.verdict == Verdict::Approve {
            info!("pipeline: gate approved draft '{}' on attempt {}", draft.title, retry + 1);

            // Format issue body with acceptance criteria checkboxes
            let criteria_md: String = draft
                .acceptance_criteria
                .iter()
                .map(|c| format!("- [ ] {c}\n"))
                .collect();

            let issue_body = format!(
                "{}\n\n## Acceptance Criteria\n\n{}\n\n---\n\
                 *Generated by QUASI Senate Loop — Phase {} — Generator model: `{}`*",
                draft.description,
                criteria_md,
                draft.phase_id,
                a2_entry.id,
            );

            // Open the GitHub issue
            let label_str = draft.label.clone();
            let issue = ctx
                .github
                .create_issue(&draft.title, &issue_body, &[&label_str])
                .await?;

            // Record event to ledger
            let _ = crate::ledger::record_event(
                "issue_generated",
                a2_entry.model,
                a2_entry.provider,
                level,
                &issue.html_url,
            )
            .await;

            // Post approval to Matrix + Fedi
            if let Some(bot) = &ctx.matrix {
                let plain = format!(
                    "✅ Issue #{} opened: {} — {}",
                    issue.number, draft.title, issue.html_url
                );
                let html = format!(
                    "<p>✅ <b>Issue #{num} opened:</b> <a href=\"{url}\">{title}</a></p>",
                    num = issue.number,
                    url = issue.html_url,
                    title = draft.title,
                );
                match bot.join_room("#senate-council:matrix.arvak.io").await {
                    Ok(room_id) => {
                        if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                            warn!("pipeline: failed to post issue approval to Matrix: {err}");
                        }
                    }
                    Err(err) => {
                        warn!("pipeline: failed to join #senate-council for approval: {err}");
                    }
                }
            }

            if let Some(fedi) = &ctx.fedi {
                let status = format!(
                    "✅ QUASI Senate Loop opened issue #{}: {} {}",
                    issue.number, draft.title, issue.html_url
                );
                if let Err(err) = fedi.post_status(&status, "public").await {
                    warn!("pipeline: fedi post approval failed: {err}");
                }
            }

            // Update state
            ctx.state.last_generate_provider = Some(a2_entry.provider.to_string());
            save_state(&ctx.state)?;

            return Ok(issue.number);
        }

        // Rejected — set up for next retry
        info!(
            "pipeline: gate rejected draft on attempt {} — reason: {}",
            retry + 1,
            verdict.reasoning
        );

        drafter_exclude.push(a2_entry.id.to_string());
        retry_feedback = Some(
            verdict
                .suggestions
                .unwrap_or_else(|| verdict.reasoning.clone()),
        );
    }

    // All retries exhausted
    if let Some(bot) = &ctx.matrix {
        let plain = "⚠️ Draft shelved after 2 rejected attempts — moving on.".to_string();
        let html = "<p>⚠️ <b>Draft shelved</b> after 2 rejected attempts — moving on.</p>"
            .to_string();
        match bot.join_room("#senate-drafts:matrix.arvak.io").await {
            Ok(room_id) => {
                let _ = bot.send_message(&room_id, &plain, &html).await;
            }
            Err(err) => {
                warn!("pipeline: failed to join #senate-drafts for shelved message: {err}");
            }
        }
    }

    Err(anyhow!("Draft rejected after 2 attempts"))
}

// ── B-track ───────────────────────────────────────────────────────────────────

/// Run the B-track pipeline for one issue: B.1 solve, B.2 review, retry up to 2×.
///
/// On approval, opens the PR. Returns the PR URL.
pub async fn run_solve_pipeline(ctx: &mut AppContext, issue_number: u32) -> Result<String> {
    info!("pipeline: starting B-track solve pipeline for issue #{issue_number}");

    // 1. Fetch the issue
    let issue = ctx.github.get_issue(issue_number).await?;
    let issue_body = issue.body.clone().unwrap_or_default();
    let issue_title = issue.title.clone();
    let issue_labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();

    // 2. Extract drafter model from issue body footer
    let drafter_model = extract_drafter_model(&issue_body);

    // 3. Build repo context (via solver module which fetches README + label files)
    // We pass the issue to solver which handles context building internally
    let counts: HashMap<String, u32> = HashMap::new();
    let last_provider = ctx.state.last_solve_provider.as_deref();

    let mut solver_exclude: Vec<String> =
        drafter_model.iter().map(|m| m.clone()).collect();
    let mut retry_feedback: Option<String> = None;

    // 4. Retry loop — up to 2 attempts
    for retry in 0..2u32 {
        info!("pipeline: B.1 solve attempt {}", retry + 1);

        let exclude_refs: Vec<&str> = solver_exclude.iter().map(|s| s.as_str()).collect();

        // B.1 — Solve
        let (solve_result, b1_entry) = crate::solver::solve_issue(
            &ctx.github,
            issue_number,
            &issue_title,
            &issue_body,
            &issue_labels,
            &exclude_refs,
            &counts,
            last_provider,
            retry_feedback.as_deref(),
            ctx.dry_run,
        )
        .await?;

        // Post solution to Matrix #senate-solutions
        if let Some(bot) = &ctx.matrix {
            let (plain, html) =
                crate::matrix::format_solution_message(issue_number, &solve_result);
            match bot.join_room("#senate-solutions:matrix.arvak.io").await {
                Ok(room_id) => {
                    if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                        warn!("pipeline: failed to post solution to Matrix: {err}");
                    }
                }
                Err(err) => {
                    warn!("pipeline: failed to join #senate-solutions: {err}");
                }
            }
        }

        // B.2 — Review (exclude drafter + solver)
        let mut reviewer_exclude = exclude_refs.clone();
        reviewer_exclude.push(b1_entry.id);

        // Build a minimal repo context for the reviewer
        // (solver already fetched context; pass what we have)
        let repo_context = "(context fetched by solver)";

        let (review_verdict, _b2_entry) = crate::reviewer::review_solution(
            &issue_title,
            &issue_body,
            &solve_result,
            repo_context,
            &reviewer_exclude,
            &counts,
            last_provider,
            ctx.dry_run,
        )
        .await?;

        // Post verdict to Matrix #senate-solutions
        if let Some(bot) = &ctx.matrix {
            let (plain, html) =
                crate::matrix::format_review_message(&review_verdict, issue_number);
            match bot.join_room("#senate-solutions:matrix.arvak.io").await {
                Ok(room_id) => {
                    if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                        warn!("pipeline: failed to post review verdict to Matrix: {err}");
                    }
                }
                Err(err) => {
                    warn!("pipeline: failed to join #senate-solutions for verdict: {err}");
                }
            }
        }

        if review_verdict.verdict == Verdict::Approve {
            info!(
                "pipeline: reviewer approved solution for #{issue_number} on attempt {}",
                retry + 1
            );

            // Apply edits via GitHub API
            let pr_url = apply_and_pr(ctx, issue_number, &issue_title, &solve_result, b1_entry).await?;

            // Record event to ledger
            let _ = crate::ledger::record_event(
                "completion",
                b1_entry.model,
                b1_entry.provider,
                0,
                &pr_url,
            )
            .await;

            // Post approval to Matrix + Fedi
            if let Some(bot) = &ctx.matrix {
                let plain = format!(
                    "✅ PR opened for #{issue_number}: {pr_url}"
                );
                let html = format!(
                    "<p>✅ <b>PR opened for #{issue_number}:</b> <a href=\"{pr_url}\">{pr_url}</a></p>"
                );
                match bot.join_room("#senate-solutions:matrix.arvak.io").await {
                    Ok(room_id) => {
                        if let Err(err) = bot.send_message(&room_id, &plain, &html).await {
                            warn!("pipeline: failed to post PR approval to Matrix: {err}");
                        }
                    }
                    Err(err) => {
                        warn!("pipeline: failed to join #senate-solutions for PR post: {err}");
                    }
                }
            }

            if let Some(fedi) = &ctx.fedi {
                let status = format!(
                    "✅ QUASI Senate Loop opened PR for issue #{issue_number}: {pr_url}"
                );
                if let Err(err) = fedi.post_status(&status, "public").await {
                    warn!("pipeline: fedi PR announcement failed: {err}");
                }
            }

            // Update state
            ctx.state.last_solve_provider = Some(b1_entry.provider.to_string());
            save_state(&ctx.state)?;

            return Ok(pr_url);
        }

        // Request changes — retry
        info!(
            "pipeline: reviewer requested changes for #{issue_number} on attempt {} — reason: {}",
            retry + 1,
            review_verdict.reasoning
        );

        solver_exclude.push(b1_entry.id.to_string());
        retry_feedback = Some(
            review_verdict
                .suggested_fix
                .unwrap_or_else(|| {
                    let issues = review_verdict.issues.join("; ");
                    if issues.is_empty() {
                        review_verdict.reasoning.clone()
                    } else {
                        format!("{}\n\nSpecific issues: {}", review_verdict.reasoning, issues)
                    }
                }),
        );
    }

    // All retries exhausted
    if let Some(bot) = &ctx.matrix {
        let plain = format!("❌ Solution for #{issue_number} failed after 2 attempts — shelved.");
        let html = format!(
            "<p>❌ <b>Solution for #{issue_number}</b> failed after 2 attempts — shelved.</p>"
        );
        match bot.join_room("#senate-solutions:matrix.arvak.io").await {
            Ok(room_id) => {
                let _ = bot.send_message(&room_id, &plain, &html).await;
            }
            Err(err) => {
                warn!("pipeline: failed to join #senate-solutions for failure message: {err}");
            }
        }
    }

    Err(anyhow!(
        "Solution for #{issue_number} rejected after 2 attempts"
    ))
}

// ── Full cycles ───────────────────────────────────────────────────────────────

/// Full cycle: draft one issue through A-track, then solve it through B-track.
pub async fn run_cycle(ctx: &mut AppContext) -> Result<()> {
    let issue_number = run_draft_pipeline(ctx).await?;
    run_solve_pipeline(ctx, issue_number).await?;
    Ok(())
}

/// Batch: draft N issues, then solve each through B-track.
pub async fn run_batch(ctx: &mut AppContext, count: u32) -> Result<()> {
    info!("pipeline: running batch of {count} cycles");

    let mut issue_numbers: Vec<u32> = Vec::new();

    // Draft N issues
    for i in 0..count {
        info!("pipeline: batch draft {}/{count}", i + 1);
        match run_draft_pipeline(ctx).await {
            Ok(n) => issue_numbers.push(n),
            Err(err) => {
                warn!("pipeline: batch draft {}/{count} failed: {err}", i + 1);
            }
        }
    }

    // Solve each drafted issue
    for (i, &issue_number) in issue_numbers.iter().enumerate() {
        info!("pipeline: batch solve {}/{} (issue #{})", i + 1, issue_numbers.len(), issue_number);
        match run_solve_pipeline(ctx, issue_number).await {
            Ok(pr_url) => {
                info!("pipeline: batch solve {} complete: {pr_url}", i + 1);
            }
            Err(err) => {
                warn!(
                    "pipeline: batch solve {}/{} (issue #{}) failed: {err}",
                    i + 1,
                    issue_numbers.len(),
                    issue_number
                );
            }
        }
    }

    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Extract the generator model identifier from the issue body footer.
///
/// Looks for: `Generator model: \`model-id\``
fn extract_drafter_model(body: &str) -> Option<String> {
    let re = Regex::new(r"Generator model:\s*`([^`]+)`").ok()?;
    let caps = re.captures(body)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Apply solve edits via the GitHub API and open a PR. Returns the PR URL.
async fn apply_and_pr(
    ctx: &mut AppContext,
    issue_number: u32,
    issue_title: &str,
    solve_result: &crate::types::SolveResult,
    b1_entry: &'static crate::types::RotationEntry,
) -> Result<String> {
    // a. Get default branch SHA
    let base_sha = ctx.github.get_default_branch_sha().await?;

    // b. Create branch
    let branch_name = format!("senate/fix-{issue_number}-{}", b1_entry.id);
    ctx.github.create_branch(&branch_name, &base_sha).await?;

    // c. Apply edits to existing files
    for edit in &solve_result.edits {
        // Fetch current file content
        match ctx.github.get_file(&edit.file, "main").await {
            Ok(fc) => {
                let new_content = fc.content.replacen(&edit.find, &edit.replace, 1);
                let commit_msg = format!(
                    "senate(fix-#{issue_number}): edit {} via {}",
                    edit.file, b1_entry.id
                );
                if let Err(err) = ctx
                    .github
                    .create_or_update_file(
                        &edit.file,
                        &new_content,
                        &commit_msg,
                        &branch_name,
                        Some(&fc.sha),
                    )
                    .await
                {
                    warn!(
                        "pipeline: apply_and_pr: failed to update '{}': {err}",
                        edit.file
                    );
                }
            }
            Err(err) => {
                warn!(
                    "pipeline: apply_and_pr: could not fetch '{}' for editing: {err}",
                    edit.file
                );
            }
        }
    }

    // d. Create new files
    for (path, content) in &solve_result.new_files {
        let commit_msg = format!(
            "senate(fix-#{issue_number}): create {} via {}",
            path, b1_entry.id
        );
        if let Err(err) = ctx
            .github
            .create_or_update_file(path, content, &commit_msg, &branch_name, None)
            .await
        {
            warn!(
                "pipeline: apply_and_pr: failed to create '{}': {err}",
                path
            );
        }
    }

    // e. Open PR
    let pr_body = format!(
        "Closes #{issue_number}\n\n\
         **Solver:** `{}`\n\
         **Reasoning:** {}\n\n\
         *Opened by QUASI Senate Loop*",
        b1_entry.id, solve_result.reasoning
    );

    let pr = ctx
        .github
        .create_pull_request(
            &format!("fix: {issue_title} (closes #{issue_number})"),
            &pr_body,
            &branch_name,
            "main",
        )
        .await?;

    Ok(pr.html_url)
}
