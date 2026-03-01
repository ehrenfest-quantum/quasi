// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Structured telemetry — one row per LLM call written to Postgres.
//!
//! The database is optional: if DATABASE_URL is not set, all writes
//! silently skip. A Postgres failure MUST NOT abort the pipeline.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// One row per LLM call, written after the downstream verdict is known.
pub struct TelemetryEntry {
    pub timestamp: DateTime<Utc>,
    pub cycle_id: Uuid,
    /// "A1_council" | "A2_drafter" | "A3_gate" | "B1_solver" | "B2_reviewer"
    pub role: String,
    /// Model ID from rotation entry (e.g., "llama3.3-groq")
    pub model_id: String,
    /// Actual model string sent to the provider API (e.g., "llama-3.3-70b-versatile")
    pub model_string: String,
    /// Provider name (e.g., "groq", "openrouter")
    pub provider: String,
    /// Base model family without provider suffix, for cross-provider grouping.
    /// "llama3.3-groq" and "llama3.3" both map to base_model "llama3.3"
    pub base_model: String,
    /// Pauli-Test level (0-4), None for council
    pub level: Option<u8>,
    /// GitHub issue number if applicable
    pub issue_number: Option<u32>,
    /// Wall-clock latency in milliseconds
    pub latency_ms: u64,
    /// Approximate input token count (chars / 4)
    pub input_tokens_approx: Option<u64>,
    /// Approximate output token count
    pub output_tokens_approx: Option<u64>,
    /// HTTP status code returned by provider
    pub http_status: Option<u16>,
    /// Retry attempts before success (0 = first attempt succeeded)
    pub retries: u32,
    /// Whether the JSON response parsed successfully
    pub json_parse_ok: Option<bool>,
    /// Downstream outcome: "approved"/"rejected"/"json_fail"/"success"/"error"
    pub downstream_verdict: Option<String>,
    /// Did x-finalized-model header match the requested model? (OpenRouter only)
    pub model_verified: Option<bool>,
    /// The x-finalized-model header value if present
    pub served_model: Option<String>,
    /// Error message if the call failed
    pub error: Option<String>,
    /// Whether this was a dry run
    pub dry_run: bool,
}

/// Strip the provider suffix from a model ID to get the canonical base model name.
///
/// "llama3.3-groq" → "llama3.3"
/// "llama3.3"      → "llama3.3" (no suffix to strip)
pub fn base_model_id(id: &str) -> String {
    for provider_name in ["openrouter", "groq", "huggingface", "fireworks", "sarvam", "mistral", "swissai", "hf", "or", "fw", "native"] {
        if let Some(base) = id.strip_suffix(&format!("-{}", provider_name)) {
            return base.to_string();
        }
    }
    id.to_string()
}

/// Connect to Postgres. Returns None if DATABASE_URL is not set.
///
/// Spawns the connection driver as a background tokio task.
pub async fn connect_db() -> Option<tokio_postgres::Client> {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return None,
    };
    match tokio_postgres::connect(&url, tokio_postgres::NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::error!("Postgres connection error: {e}");
                }
            });
            Some(client)
        }
        Err(e) => {
            tracing::warn!("Could not connect to Postgres (telemetry disabled): {e}");
            None
        }
    }
}

/// Write one telemetry entry. Silently skips if db is None or on error.
pub async fn record_telemetry(db: &Option<tokio_postgres::Client>, entry: &TelemetryEntry) {
    let Some(db) = db else { return };
    let result = db
        .execute(
            "INSERT INTO senate_telemetry (
                timestamp, cycle_id, role, model_id, model_string, provider, base_model,
                level, issue_number, latency_ms, input_tokens_approx, output_tokens_approx,
                http_status, retries, json_parse_ok, downstream_verdict,
                model_verified, served_model, error, dry_run
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)",
            &[
                &entry.timestamp,
                &entry.cycle_id,
                &entry.role,
                &entry.model_id,
                &entry.model_string,
                &entry.provider,
                &entry.base_model,
                &entry.level.map(|l| l as i16),
                &entry.issue_number.map(|n| n as i32),
                &(entry.latency_ms as i64),
                &entry.input_tokens_approx.map(|t| t as i64),
                &entry.output_tokens_approx.map(|t| t as i64),
                &entry.http_status.map(|s| s as i16),
                &(entry.retries as i32),
                &entry.json_parse_ok,
                &entry.downstream_verdict,
                &entry.model_verified,
                &entry.served_model,
                &entry.error,
                &entry.dry_run,
            ],
        )
        .await;
    if let Err(e) = result {
        tracing::warn!("Failed to write telemetry row: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_model_id_strips_groq() {
        assert_eq!(base_model_id("llama3.3-groq"), "llama3.3");
    }

    #[test]
    fn test_base_model_id_strips_hf() {
        assert_eq!(base_model_id("deepseek-v3-hf"), "deepseek-v3");
    }

    #[test]
    fn test_base_model_id_no_suffix() {
        assert_eq!(base_model_id("deepseek-v3"), "deepseek-v3");
    }

    #[test]
    fn test_base_model_id_strips_or() {
        assert_eq!(base_model_id("qwen3-32b-or"), "qwen3-32b");
    }

    #[test]
    fn test_base_model_id_native() {
        assert_eq!(base_model_id("mistral-small-native"), "mistral-small");
    }

    #[test]
    fn test_telemetry_entry_fields() {
        let entry = TelemetryEntry {
            timestamp: Utc::now(),
            cycle_id: Uuid::new_v4(),
            role: "A2_drafter".to_string(),
            model_id: "llama3.3-groq".to_string(),
            model_string: "llama-3.3-70b-versatile".to_string(),
            provider: "groq".to_string(),
            base_model: base_model_id("llama3.3-groq"),
            level: Some(2),
            issue_number: None,
            latency_ms: 1500,
            input_tokens_approx: Some(2000),
            output_tokens_approx: Some(500),
            http_status: Some(200),
            retries: 0,
            json_parse_ok: Some(true),
            downstream_verdict: Some("approved".to_string()),
            model_verified: None,
            served_model: None,
            error: None,
            dry_run: false,
        };
        assert_eq!(entry.base_model, "llama3.3");
        assert_eq!(entry.role, "A2_drafter");
    }
}
