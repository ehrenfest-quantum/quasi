// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Akkoma / Mastodon-compatible ActivityPub status poster.
//!
//! Fedi integration is entirely optional — if the required environment variables
//! are absent the daemon continues without posting to the federated timeline.

use anyhow::Result;
use tracing::warn;

pub struct FediClient {
    instance_url: String,
    access_token: String,
    client: reqwest::Client,
}

impl FediClient {
    /// Construct a client from environment variables.
    ///
    /// Returns `None` when either `AKKOMA_INSTANCE_URL` or
    /// `AKKOMA_ACCESS_TOKEN` is not set.
    pub fn try_new() -> Option<Self> {
        let instance_url = std::env::var("AKKOMA_INSTANCE_URL").ok()?;
        let access_token = std::env::var("AKKOMA_ACCESS_TOKEN").ok()?;

        if instance_url.trim().is_empty() || access_token.trim().is_empty() {
            warn!("fedi: AKKOMA_INSTANCE_URL or AKKOMA_ACCESS_TOKEN is empty — skipping Fedi");
            return None;
        }

        Some(Self {
            instance_url,
            access_token,
            client: reqwest::Client::new(),
        })
    }

    /// Post a status to the Akkoma / Mastodon-compatible API.
    ///
    /// Errors are logged as warnings but never propagated — Fedi is optional
    /// output and must never block the pipeline.
    pub async fn post_status(&self, text: &str, visibility: &str) -> Result<()> {
        let url = format!("{}/api/v1/statuses", self.instance_url);

        let params = [("status", text), ("visibility", visibility)];

        let result = self
            .client
            .post(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .form(&params)
            .send()
            .await;

        match result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!(
                        "fedi: post_status returned non-success status {}",
                        resp.status()
                    );
                }
            }
            Err(err) => {
                warn!("fedi: post_status failed: {err}");
            }
        }

        Ok(())
    }
}
