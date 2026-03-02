// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Raw Matrix Client-Server API bot.
//!
//! Uses `reqwest` directly — no matrix-sdk dependency. Matrix is output-only:
//! the daemon posts notifications but never reads from rooms.

use anyhow::{anyhow, Context, Result};
use tracing::warn;
use uuid::Uuid;

use crate::types::{Charter, GateVerdict, IssueDraft, ReviewVerdict, SolveResult};

/// Room aliases the Senate daemon joins on startup.
pub const SENATE_ROOMS: &[&str] = &[
    "#senate-council:paulsboutique.hal-contract.org",
    "#senate-drafts:paulsboutique.hal-contract.org",
    "#senate-solutions:paulsboutique.hal-contract.org",
    "#senate-ledger:paulsboutique.hal-contract.org",
];

pub struct MatrixBot {
    homeserver: String,
    access_token: String,
    client: reqwest::Client,
}

impl MatrixBot {
    /// Attempt to login using environment variables.
    ///
    /// Reads `MATRIX_HOMESERVER`, `MATRIX_USERNAME`, and `MATRIX_PASSWORD`.
    /// Returns `None` with a warning log when any variable is missing or empty.
    pub async fn try_login() -> Option<Self> {
        let homeserver = std::env::var("MATRIX_HOMESERVER").ok()?;
        let username = std::env::var("MATRIX_USERNAME").ok()?;
        let password = std::env::var("MATRIX_PASSWORD").ok()?;

        if homeserver.trim().is_empty() || username.trim().is_empty() || password.trim().is_empty() {
            warn!("matrix: one or more Matrix env vars are empty — skipping Matrix bot");
            return None;
        }

        match Self::login(&homeserver, &username, &password).await {
            Ok(bot) => Some(bot),
            Err(err) => {
                warn!("matrix: login failed: {err} — skipping Matrix bot");
                None
            }
        }
    }

    /// Login to the Matrix homeserver and return a bot instance.
    pub async fn login(homeserver: &str, username: &str, password: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("matrix: build reqwest client")?;
        let url = format!("{}/_matrix/client/v3/login", homeserver);

        let payload = serde_json::json!({
            "type": "m.login.password",
            "identifier": {
                "type": "m.id.user",
                "user": username
            },
            "password": password
        });

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("matrix: login request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("matrix: login returned {status}: {body}"));
        }

        let body: serde_json::Value = resp.json().await.context("matrix: login deserialise")?;
        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("matrix: login response missing access_token"))?
            .to_string();

        Ok(Self {
            homeserver: homeserver.to_string(),
            access_token,
            client,
        })
    }

    /// Join a Matrix room by alias. Returns the canonical room ID.
    pub async fn join_room(&self, room_alias: &str) -> Result<String> {
        let encoded = urlencoding::encode(room_alias);
        let url = format!(
            "{}/_matrix/client/v3/join/{}",
            self.homeserver, encoded
        );

        let resp = self
            .client
            .post(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("matrix: join_room request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("matrix: join_room returned {status}: {body}"));
        }

        let body: serde_json::Value = resp.json().await.context("matrix: join_room deserialise")?;
        let room_id = body
            .get("room_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("matrix: join_room response missing room_id"))?
            .to_string();

        Ok(room_id)
    }

    /// Send an HTML-formatted message to a Matrix room.
    pub async fn send_message(&self, room_id: &str, body: &str, formatted_body: &str) -> Result<()> {
        let txn_id = Uuid::new_v4();
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.homeserver, room_id, txn_id
        );

        let payload = serde_json::json!({
            "msgtype": "m.text",
            "body": body,
            "format": "org.matrix.custom.html",
            "formatted_body": formatted_body
        });

        let resp = self
            .client
            .put(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .json(&payload)
            .send()
            .await
            .context("matrix: send_message request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("matrix: send_message returned {status}: {body_text}"));
        }

        Ok(())
    }
}

// ── Message formatters ────────────────────────────────────────────────────────

/// Format a Phase Charter for posting to #senate-council.
pub fn format_charter_message(charter: &Charter, model: &str) -> (String, String) {
    let priorities: String = charter
        .priorities
        .iter()
        .map(|p| format!("  {}. {} — {}", p.rank, p.area, p.description))
        .collect::<Vec<_>>()
        .join("\n");

    let plain = format!(
        "🏛️ Architecture Council — Phase {}\n\
         Model: {}\n\
         Goal: {}\n\
         Frontier Level: L{}\n\
         Priorities:\n{}\n\
         Quota: {} issues this phase",
        charter.phase_id,
        model,
        charter.goal,
        charter.frontier_level,
        priorities,
        charter.quota.total_issues_this_phase,
    );

    let priorities_html: String = charter
        .priorities
        .iter()
        .map(|p| {
            format!(
                "<li><b>{}</b> — {} (max {} issues, L{})</li>",
                p.area, p.description, p.max_issues, p.level
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        "<p>🏛️ <b>Architecture Council — Phase {phase}</b></p>\
         <p><b>Model:</b> {model}</p>\
         <p><b>Goal:</b> {goal}</p>\
         <p><b>Frontier Level:</b> L{level}</p>\
         <p><b>Priorities:</b></p>\
         <ul>{priorities_html}</ul>\
         <p><b>Quota:</b> {quota} issues this phase</p>",
        phase = charter.phase_id,
        model = model,
        goal = charter.goal,
        level = charter.frontier_level,
        priorities_html = priorities_html,
        quota = charter.quota.total_issues_this_phase,
    );

    (plain, html)
}

/// Format an issue draft for posting to #senate-drafts.
pub fn format_draft_message(draft: &IssueDraft) -> (String, String) {
    let criteria: String = draft
        .acceptance_criteria
        .iter()
        .map(|c| format!("  - {c}"))
        .collect::<Vec<_>>()
        .join("\n");

    let plain = format!(
        "📝 Draft Issue — {}\n\
         Model: {}\n\
         Label: {}\n\
         Phase: {}\n\
         Description: {}\n\
         Acceptance Criteria:\n{}",
        draft.title,
        draft.drafter_model,
        draft.label,
        draft.phase_id,
        draft.description,
        criteria,
    );

    let criteria_html: String = draft
        .acceptance_criteria
        .iter()
        .map(|c| format!("<li>{c}</li>"))
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        "<p>📝 <b>Draft Issue — {title}</b></p>\
         <p><b>Model:</b> {model} | <b>Label:</b> {label} | <b>Phase:</b> {phase}</p>\
         <p>{description}</p>\
         <p><b>Acceptance Criteria:</b></p>\
         <ul>{criteria_html}</ul>",
        title = draft.title,
        model = draft.drafter_model,
        label = draft.label,
        phase = draft.phase_id,
        description = draft.description,
        criteria_html = criteria_html,
    );

    (plain, html)
}

/// Format a gate verdict for posting to #senate-drafts.
pub fn format_verdict_message(verdict: &GateVerdict, draft_title: &str) -> (String, String) {
    let emoji = match verdict.verdict {
        crate::types::Verdict::Approve => "✅",
        crate::types::Verdict::Reject => "❌",
        crate::types::Verdict::RequestChanges => "🔄",
    };

    let suggestions_text = verdict
        .suggestions
        .as_deref()
        .map(|s| format!("\nSuggestions: {s}"))
        .unwrap_or_default();

    let plain = format!(
        "{emoji} Gate Verdict for \"{draft_title}\"\n\
         Reviewer: {model}\n\
         Verdict: {verdict}\n\
         Reasoning: {reasoning}{suggestions}",
        emoji = emoji,
        draft_title = draft_title,
        model = verdict.reviewer_model,
        verdict = verdict.verdict,
        reasoning = verdict.reasoning,
        suggestions = suggestions_text,
    );

    let suggestions_html = verdict
        .suggestions
        .as_deref()
        .map(|s| format!("<p><b>Suggestions:</b> {s}</p>"))
        .unwrap_or_default();

    let html = format!(
        "<p>{emoji} <b>Gate Verdict for &quot;{draft_title}&quot;</b></p>\
         <p><b>Reviewer:</b> {model} | <b>Verdict:</b> {verdict}</p>\
         <p><b>Reasoning:</b> {reasoning}</p>{suggestions_html}",
        emoji = emoji,
        draft_title = draft_title,
        model = verdict.reviewer_model,
        verdict = verdict.verdict,
        reasoning = verdict.reasoning,
        suggestions_html = suggestions_html,
    );

    (plain, html)
}

/// Format a solve result for posting to #senate-solutions.
pub fn format_solution_message(issue_number: u32, solve_result: &SolveResult) -> (String, String) {
    let edit_count = solve_result.edits.len();
    let new_file_count = solve_result.new_files.len();

    let files: Vec<String> = solve_result.edits.iter().map(|e| e.file.clone()).collect();
    let new_files: Vec<String> = solve_result.new_files.keys().cloned().collect();

    let files_str = if files.is_empty() {
        "(no edits)".to_string()
    } else {
        files.join(", ")
    };

    let plain = format!(
        "🔧 Solution for #{issue_number}\n\
         Solver: {model}\n\
         Reasoning: {reasoning}\n\
         Edits: {edit_count} ({files})\n\
         New files: {new_file_count}",
        issue_number = issue_number,
        model = solve_result.solver_model,
        reasoning = solve_result.reasoning,
        edit_count = edit_count,
        files = files_str,
        new_file_count = new_file_count,
    );

    let files_html: String = files
        .iter()
        .chain(new_files.iter())
        .map(|f| format!("<li><code>{f}</code></li>"))
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        "<p>🔧 <b>Solution for #{issue_number}</b></p>\
         <p><b>Solver:</b> {model}</p>\
         <p><b>Reasoning:</b> {reasoning}</p>\
         <p><b>Files touched ({edit_count} edits, {new_file_count} new):</b></p>\
         <ul>{files_html}</ul>",
        issue_number = issue_number,
        model = solve_result.solver_model,
        reasoning = solve_result.reasoning,
        edit_count = edit_count,
        new_file_count = new_file_count,
        files_html = files_html,
    );

    (plain, html)
}

/// Format a review verdict for posting to #senate-solutions.
pub fn format_review_message(verdict: &ReviewVerdict, issue_number: u32) -> (String, String) {
    let emoji = match verdict.verdict {
        crate::types::Verdict::Approve => "✅",
        crate::types::Verdict::Reject => "❌",
        crate::types::Verdict::RequestChanges => "🔄",
    };

    let issues_text = if verdict.issues.is_empty() {
        String::new()
    } else {
        let items = verdict
            .issues
            .iter()
            .map(|i| format!("  - {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\nIssues:\n{items}")
    };

    let fix_text = verdict
        .suggested_fix
        .as_deref()
        .map(|s| format!("\nSuggested fix: {s}"))
        .unwrap_or_default();

    let plain = format!(
        "{emoji} Review Verdict for #{issue_number}\n\
         Reviewer: {model}\n\
         Verdict: {verdict}\n\
         Reasoning: {reasoning}{issues}{fix}",
        emoji = emoji,
        issue_number = issue_number,
        model = verdict.reviewer_model,
        verdict = verdict.verdict,
        reasoning = verdict.reasoning,
        issues = issues_text,
        fix = fix_text,
    );

    let issues_html = if verdict.issues.is_empty() {
        String::new()
    } else {
        let items = verdict
            .issues
            .iter()
            .map(|i| format!("<li>{i}</li>"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("<p><b>Issues:</b></p><ul>{items}</ul>")
    };

    let fix_html = verdict
        .suggested_fix
        .as_deref()
        .map(|s| format!("<p><b>Suggested fix:</b> {s}</p>"))
        .unwrap_or_default();

    let html = format!(
        "<p>{emoji} <b>Review Verdict for #{issue_number}</b></p>\
         <p><b>Reviewer:</b> {model} | <b>Verdict:</b> {verdict}</p>\
         <p><b>Reasoning:</b> {reasoning}</p>{issues_html}{fix_html}",
        emoji = emoji,
        issue_number = issue_number,
        model = verdict.reviewer_model,
        verdict = verdict.verdict,
        reasoning = verdict.reasoning,
        issues_html = issues_html,
        fix_html = fix_html,
    );

    (plain, html)
}

// ── urlencoding helper (inline, no extra dep) ─────────────────────────────────

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~' => out.push(byte as char),
                b => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }
}
