// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! B.1 Issue Solver role.

use anyhow::Result;
use std::collections::HashMap;

use crate::github::GitHubClient;
use crate::types::{FileEdit, Role, RotationEntry, SolveResult};

/// Private raw struct for LLM JSON response.
#[derive(serde::Deserialize)]
struct SolveResultRaw {
    reasoning: String,
    #[serde(default)]
    edits: Vec<FileEdit>,
    #[serde(default)]
    new_files: HashMap<String, String>,
}

/// Solve an issue (B.1). Returns the proposed solution and the rotation entry used.
///
/// * `exclude` — model IDs to exclude (at minimum: the drafter's model id)
/// * `retry_feedback` — if retry, include review feedback
#[allow(clippy::too_many_arguments)]
pub async fn solve_issue(
    github: &GitHubClient,
    issue_number: u32,
    issue_title: &str,
    issue_body: &str,
    issue_labels: &[String],
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
    retry_feedback: Option<&str>,
    dry_run: bool,
) -> Result<(SolveResult, &'static RotationEntry, crate::provider::CallResult, String)> {
    // 1. Pick model
    let entry = crate::rotation::pick_model(&Role::B1Solver, exclude, counts, last_provider)?;

    // 2. Build repo context from github
    let mut context_parts: Vec<String> = Vec::new();

    // Always fetch README.md
    if let Ok(fc) = github.get_file("README.md", "main").await {
        context_parts.push(format!("#### README.md\n```\n{}\n```", fc.content));
    }

    // Always fetch ARCHITECTURE.md
    if let Ok(fc) = github.get_file("ARCHITECTURE.md", "main").await {
        context_parts.push(format!("#### ARCHITECTURE.md\n```\n{}\n```", fc.content));
    }

    // Label-specific extra files
    let label_names: Vec<&str> = issue_labels.iter().map(|l| l.as_str()).collect();

    if label_names.contains(&"compiler") {
        if let Ok(fc) = github.get_file("spec/ehrenfest-v0.1.cddl", "main").await {
            context_parts.push(format!(
                "#### spec/ehrenfest-v0.1.cddl\n```\n{}\n```",
                fc.content
            ));
        }
        // Afana compiler source (Rust crate) — essential so the solver can see
        // existing modules, data structures, and test patterns.
        for path in &[
            "afana/src/lib.rs",
            "afana/src/ast.rs",
            "afana/src/parser.rs",
            "afana/src/emit.rs",
            "afana/src/optimize.rs",
        ] {
            if let Ok(fc) = github.get_file(path, "main").await {
                context_parts.push(format!("#### {path}\n```rust\n{}\n```", fc.content));
            }
        }
    }

    if label_names.contains(&"specification") {
        if let Ok(fc) = github.get_file("spec/ehrenfest-v0.1.cddl", "main").await {
            context_parts.push(format!(
                "#### spec/ehrenfest-v0.1.cddl\n```\n{}\n```",
                fc.content
            ));
        }
        if let Ok(fc) = github.get_file("docs/BENCHMARK.md", "main").await {
            context_parts.push(format!(
                "#### docs/BENCHMARK.md\n```\n{}\n```",
                fc.content
            ));
        }
    }

    let mut repo_context = context_parts.join("\n\n");

    // 3. Build solver user prompt
    let mut user = crate::prompts::solver_user_prompt(issue_title, issue_body, &repo_context);

    // 4. Append retry feedback if present
    if let Some(feedback) = retry_feedback {
        user.push_str(&format!(
            "\n\n## Previous review feedback\n{feedback}\n\nPlease revise to address these issues."
        ));
    }

    // 5. Apply context window truncation if entry.max_context is set
    if let Some(max_ctx) = entry.max_context {
        let max_bytes = max_ctx as usize * 4; // rough chars-per-token estimate
        if user.len() > max_bytes {
            tracing::warn!(
                "solver: truncating user prompt from {} to ~{} chars for model '{}' (max_context={})",
                user.len(),
                max_bytes,
                entry.id,
                max_ctx,
            );
            // Truncate repo_context first to preserve the issue description
            let budget = max_bytes.saturating_sub(user.len() - repo_context.len());
            if budget < repo_context.len() {
                repo_context.truncate(budget);
                user = crate::prompts::solver_user_prompt(issue_title, issue_body, &repo_context);
                if let Some(feedback) = retry_feedback {
                    user.push_str(&format!(
                        "\n\n## Previous review feedback\n{feedback}\n\nPlease revise to address these issues."
                    ));
                }
            }
        }
    }

    // 6. Dry-run path
    if dry_run {
        println!(
            "[dry-run] solver: would call model '{}' for issue #{} {:?}",
            entry.id, issue_number, issue_title,
        );
        let placeholder = SolveResult {
            reasoning: "Dry-run placeholder reasoning.".to_string(),
            edits: vec![],
            new_files: HashMap::new(),
            solver_model: entry.id.to_string(),
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
        return Ok((placeholder, entry, dummy_call, repo_context));
    }

    // 7. Call the LLM
    let system = crate::prompts::solver_system_prompt();
    let max_tokens = entry.max_tokens.unwrap_or(8192);
    let call_result = crate::provider::call_model(entry, system, &user, 0.2, max_tokens).await?;
    let raw = call_result.content.clone();

    // 8. Parse raw response — map failure to ParseFailure so pipeline can write telemetry.
    let raw_result = crate::provider::parse_json_response::<SolveResultRaw>(&raw)
        .map_err(|e| crate::provider::ParseFailure {
            call: call_result.clone(),
            entry,
            error: e.to_string(),
        })?;

    // 9. Convert to SolveResult
    let result = SolveResult {
        reasoning: raw_result.reasoning,
        edits: raw_result.edits,
        new_files: raw_result.new_files,
        solver_model: entry.id.to_string(),
    };

    tracing::info!(
        "Solver complete: model={} issue=#{} edits={} new_files={}",
        entry.id,
        issue_number,
        result.edits.len(),
        result.new_files.len(),
    );

    // 10. Return — include repo_context so the reviewer can reuse it
    Ok((result, entry, call_result, repo_context))
}
