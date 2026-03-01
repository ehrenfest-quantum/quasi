// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! `quasi-senate` binary entry point — CLI dispatch for the Senate Loop daemon.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use regex::Regex;

use quasi_senate::config::ROTATION;
use quasi_senate::fedi::FediClient;
use quasi_senate::github::GitHubClient;
use quasi_senate::matrix::MatrixBot;
use quasi_senate::pipeline::{AppContext, run_batch, run_council, run_cycle, run_draft_pipeline, run_solve_pipeline};
use quasi_senate::state::{load_state, save_state};
use quasi_senate::telemetry::init_telemetry;
use quasi_senate::telemetry_log;

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "quasi-senate",
    about = "QUASI Senate Loop governance daemon",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Perform a dry run — no LLM calls, no GitHub mutations
    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run Architecture Council session (A.1)
    Council,

    /// Draft one issue through the A-track pipeline
    Draft,

    /// Solve one open issue through the B-track pipeline
    Solve {
        /// GitHub issue number to solve (defaults to oldest open Senate-generated issue)
        #[arg(long)]
        issue: Option<u32>,
    },

    /// Full cycle: draft one issue, then solve one open issue
    Cycle,

    /// Batch: draft and solve N issues
    Batch {
        /// Number of issues to draft and solve
        #[arg(long, default_value = "5")]
        count: u32,
    },

    /// Show the current phase charter
    Charter,

    /// Amend the current charter (admin emergency override)
    Amend {
        /// JSON merge-patch to apply to the current charter
        #[arg(long)]
        patch: String,
    },

    /// List eligible models and their roles
    Models,

    /// Show current state (retries, quotas, last providers)
    Status,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load .env file (ignore error if absent)
    dotenvy::dotenv().ok();

    // 2. Initialize tracing
    init_telemetry();

    // 3. Parse CLI
    let cli = Cli::parse();

    // 4. Build AppContext
    let github_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| anyhow!("GITHUB_TOKEN environment variable is not set"))?;

    let github = GitHubClient::new(github_token, "ehrenfest-quantum/quasi".to_string());
    let matrix = MatrixBot::try_login().await;
    let fedi = FediClient::try_new();
    let state = load_state()?;
    let dry_run = cli.dry_run;

    let db = telemetry_log::connect_db().await;

    let mut ctx = AppContext {
        github,
        matrix,
        fedi,
        state,
        dry_run,
        db,
    };

    // Run DB migration on startup
    if let Some(db) = &ctx.db {
        let migration = include_str!("../migrations/001_telemetry.sql");
        if let Err(e) = db.batch_execute(migration).await {
            tracing::warn!("Migration warning (may already exist): {e}");
        }
    }

    // 5. Dispatch
    match cli.command {
        Command::Council => {
            let charter = run_council(&mut ctx).await?;
            save_state(&ctx.state)?;
            println!("Council session complete: phase_id={}", charter.phase_id);
        }

        Command::Draft => {
            let issue_number = run_draft_pipeline(&mut ctx).await?;
            println!("Draft pipeline complete: opened issue #{issue_number}");
        }

        Command::Solve { issue } => {
            let issue_number = match issue {
                Some(n) => n,
                None => find_oldest_senate_issue(&mut ctx).await?,
            };
            let pr_url = run_solve_pipeline(&mut ctx, issue_number).await?;
            println!("Solve pipeline complete: {pr_url}");
        }

        Command::Cycle => {
            run_cycle(&mut ctx).await?;
            println!("Cycle complete.");
        }

        Command::Batch { count } => {
            run_batch(&mut ctx, count).await?;
            println!("Batch of {count} complete.");
        }

        Command::Charter => {
            match &ctx.state.current_charter {
                Some(charter_json) => println!("{charter_json}"),
                None => println!("No charter found. Run `quasi-senate council` first."),
            }
        }

        Command::Amend { patch } => {
            let current_json = ctx
                .state
                .current_charter
                .as_deref()
                .ok_or_else(|| anyhow!("No current charter to amend."))?;

            // Parse current charter and patch
            let mut current: serde_json::Value = serde_json::from_str(current_json)
                .map_err(|e| anyhow!("Failed to parse current charter: {e}"))?;
            let patch_val: serde_json::Value = serde_json::from_str(&patch)
                .map_err(|e| anyhow!("Failed to parse patch JSON: {e}"))?;

            // Apply JSON merge patch (RFC 7396)
            json_merge_patch(&mut current, &patch_val);

            let updated_json = serde_json::to_string_pretty(&current)?;
            ctx.state.current_charter = Some(updated_json.clone());
            save_state(&ctx.state)?;

            println!("Charter amended successfully.");
            println!("{updated_json}");
        }

        Command::Models => {
            print_models_table();
        }

        Command::Status => {
            let status_json = serde_json::to_string_pretty(&ctx.state)?;
            println!("{status_json}");
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Find the oldest open GitHub issue that was generated by the Senate Loop.
///
/// A Senate-generated issue has a footer matching `Generator model: \`...\``.
async fn find_oldest_senate_issue(ctx: &mut AppContext) -> Result<u32> {
    let issues = ctx.github.list_open_issues(40).await?;
    let re = Regex::new(r"Generator model:\s*`([^`]+)`")?;

    // Issues are returned newest-first; reverse to get oldest-first.
    let senate_issues: Vec<_> = issues
        .into_iter()
        .filter(|issue| {
            issue
                .body
                .as_deref()
                .map(|body| re.is_match(body))
                .unwrap_or(false)
        })
        .collect();

    if senate_issues.is_empty() {
        return Err(anyhow!(
            "No open Senate-generated issues found. Draft one first with `quasi-senate draft`."
        ));
    }

    // Pick the one with the lowest issue number (oldest)
    let oldest = senate_issues
        .into_iter()
        .min_by_key(|i| i.number)
        .expect("non-empty Vec has a min element");

    tracing::info!(
        "solve: auto-selected oldest Senate issue #{}: {}",
        oldest.number,
        oldest.title
    );

    Ok(oldest.number)
}

/// Print a formatted table of eligible models and their roles.
fn print_models_table() {
    println!(
        "  {:<22} {:<14} {:<36} {:<22} {}",
        "ID", "Provider", "Roles", "License", "Origin"
    );
    println!("  {}", "-".repeat(120));

    for entry in ROTATION {
        let roles: String = entry
            .roles
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        println!(
            "  {:<22} {:<14} {:<36} {:<22} {}",
            entry.id, entry.provider, roles, entry.license, entry.origin
        );
    }
}

/// Apply a JSON merge patch (RFC 7396) in place.
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target.is_object(), patch.is_object()) {
        (true, true) => {
            let target_map = target.as_object_mut().unwrap();
            let patch_map = patch.as_object().unwrap();
            for (key, patch_val) in patch_map {
                if patch_val.is_null() {
                    target_map.remove(key);
                } else {
                    let entry = target_map
                        .entry(key)
                        .or_insert(serde_json::Value::Null);
                    json_merge_patch(entry, patch_val);
                }
            }
        }
        _ => {
            *target = patch.clone();
        }
    }
}
