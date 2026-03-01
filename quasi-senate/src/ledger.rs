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

const INBOX_URL: &str = "https://gawain.valiant-quantum.com/quasi-board/inbox";

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
