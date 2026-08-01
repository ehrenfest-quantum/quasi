// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ActivityStreams event recorder — posts to the quasi-board inbox.
//!
//! Ledger failures are non-fatal: errors are logged as warnings and the
//! function always returns `Ok(())` so the pipeline is never aborted.

use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tracing::warn;

use crate::types::IssueDraft;

/// quasi-board ActivityPub inbox — shared by ledger events and A-track proposals.
pub const INBOX_URL: &str = "https://gawain.valiant-quantum.com/quasi-board/inbox";

/// ActivityPub actor id used when the senate proposes work to the board.
pub const SENATE_ACTOR_ID: &str = "https://gawain.valiant-quantum.com/quasi-senate";

/// Post an ActivityStreams `Create` event to the quasi-board inbox.
///
/// The function succeeds even when the HTTP request fails — errors are
/// emitted as `tracing::warn!` messages so that ledger outages do not
/// block the pipeline.
pub async fn record_event(
    event_type: &str,
    model: &str,
    provider: &str,
    level: u8,
    url: &str,
) -> Result<()> {
    let published = Utc::now().to_rfc3339();

    let body = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "quasi:type": event_type,
        "quasi:level": level,
        "quasi:generator_model": model,
        "quasi:generator_provider": provider,
        "quasi:issueUrl": url,
        "published": published,
    });

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(err) => {
            warn!("ledger: failed to build HTTP client: {err}");
            return Ok(());
        }
    };

    match client.post(INBOX_URL).json(&body).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                warn!(
                    "ledger: inbox returned non-success status {} for event '{event_type}'",
                    resp.status()
                );
            }
        }
        Err(err) => {
            warn!("ledger: failed to POST event '{event_type}' to inbox: {err}");
        }
    }

    Ok(())
}

// ── A-track proposal submission (quasi:Propose) ───────────────────────────────

/// Outcome of submitting a `quasi:Propose` activity to the board inbox.
///
/// Rejections (400/409/429) are the board's complexity gate doing its job —
/// they are final answers, not errors to retry around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOutcome {
    /// 202 — proposal queued; carries the board's proposal id (e.g. "prop-042").
    Queued(String),
    /// 400 — rejected as trivial or malformed; carries the board's message.
    Rejected(String),
    /// 409 — near-duplicate of a pending proposal; the message names which one.
    Duplicate(String),
    /// 429 — L0 proposal cap reached.
    L0Cap(String),
    /// Network error or an unexpected status code.
    Failed(String),
}

/// Raw inbox response used by the injectable HTTP seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxResponse {
    pub status: u16,
    pub body: String,
}

/// Build the ActivityStreams `quasi:Propose` body for an approved draft.
pub fn build_proposal_body(draft: &IssueDraft, rationale: &str) -> serde_json::Value {
    json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "quasi:Propose",
        "actor": SENATE_ACTOR_ID,
        "object": {
            "quasi:title": draft.title,
            "quasi:description": draft.description,
            "quasi:estimatedEffort": draft.estimated_effort,
            "quasi:affectedComponents": draft.affected_components,
            "quasi:successCriteria": draft.acceptance_criteria,
            "quasi:level": "L1",
            "quasi:rationale": rationale,
        }
    })
}

/// Classify the board inbox response into a [`ProposalOutcome`].
///
/// FastAPI error responses carry the gate's message in `detail`; fall back to
/// the raw body when it isn't JSON.
pub fn classify_response(status: u16, body: &str) -> ProposalOutcome {
    let detail = || {
        serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("detail")
                    .and_then(|d| d.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| body.to_string())
    };
    match status {
        202 => {
            let id = serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("id").and_then(|i| i.as_str().map(str::to_string)))
                .unwrap_or_else(|| "(unknown proposal id)".to_string());
            ProposalOutcome::Queued(id)
        }
        400 => ProposalOutcome::Rejected(detail()),
        409 => ProposalOutcome::Duplicate(detail()),
        429 => ProposalOutcome::L0Cap(detail()),
        other => ProposalOutcome::Failed(format!(
            "board inbox returned unexpected status {other}: {body}"
        )),
    }
}

/// Submit a proposal with an injected HTTP poster so the response handling is
/// testable without network access. `poster` receives the proposal body and
/// returns the inbox response (or an error string on transport failure).
pub async fn submit_proposal_with<F, Fut>(poster: F, body: serde_json::Value) -> ProposalOutcome
where
    F: FnOnce(serde_json::Value) -> Fut,
    Fut: std::future::Future<Output = std::result::Result<InboxResponse, String>>,
{
    match poster(body).await {
        Ok(resp) => classify_response(resp.status, &resp.body),
        Err(err) => ProposalOutcome::Failed(format!("board inbox request failed: {err}")),
    }
}

/// Default HTTP poster: POST the activity to the quasi-board inbox.
async fn http_post_proposal(
    body: serde_json::Value,
) -> std::result::Result<InboxResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let resp = client
        .post(INBOX_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    Ok(InboxResponse { status, body })
}

/// Post an approved A-track draft to the quasi-board proposal queue.
pub async fn submit_proposal(draft: &IssueDraft, rationale: &str) -> ProposalOutcome {
    let body = build_proposal_body(draft, rationale);
    submit_proposal_with(http_post_proposal, body).await
}
