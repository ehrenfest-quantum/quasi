// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Model health-check: probe every ROTATION entry with a minimal LLM call.

use std::fmt;
use std::time::Instant;

use tokio::time::{timeout, Duration};
use tracing::{info, warn};

use crate::config::{get_provider, ROTATION};
use crate::provider::call_model;
use crate::types::RotationEntry;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeStatus {
    Ok,
    Fail,
    Skipped,
}

impl fmt::Display for ProbeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeStatus::Ok => write!(f, "ok"),
            ProbeStatus::Fail => write!(f, "FAIL"),
            ProbeStatus::Skipped => write!(f, "skip"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub model_id: String,
    pub provider: String,
    pub status: ProbeStatus,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

// ── Core ─────────────────────────────────────────────────────────────────────

/// Probe a single model with a minimal health-check prompt.
async fn probe_model(entry: &'static RotationEntry, timeout_secs: u64) -> ProbeResult {
    // Check if the provider's API key is available.
    let provider_cfg = match get_provider(entry.provider) {
        Some(p) => p,
        None => {
            return ProbeResult {
                model_id: entry.id.to_string(),
                provider: entry.provider.to_string(),
                status: ProbeStatus::Skipped,
                latency_ms: None,
                error: Some(format!("unknown provider '{}'", entry.provider)),
            };
        }
    };

    match std::env::var(provider_cfg.env_var) {
        Ok(v) if !v.trim().is_empty() => {}
        _ => {
            return ProbeResult {
                model_id: entry.id.to_string(),
                provider: entry.provider.to_string(),
                status: ProbeStatus::Skipped,
                latency_ms: None,
                error: Some(format!("{} not set", provider_cfg.env_var)),
            };
        }
    }

    let start = Instant::now();
    let result = timeout(
        Duration::from_secs(timeout_secs),
        call_model(entry, "You are a health check.", "Say OK", 0.0, 5),
    )
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(_call)) => ProbeResult {
            model_id: entry.id.to_string(),
            provider: entry.provider.to_string(),
            status: ProbeStatus::Ok,
            latency_ms: Some(latency_ms),
            error: None,
        },
        Ok(Err(e)) => ProbeResult {
            model_id: entry.id.to_string(),
            provider: entry.provider.to_string(),
            status: ProbeStatus::Fail,
            latency_ms: Some(latency_ms),
            error: Some(format!("{e:#}")),
        },
        Err(_) => ProbeResult {
            model_id: entry.id.to_string(),
            provider: entry.provider.to_string(),
            status: ProbeStatus::Fail,
            latency_ms: Some(latency_ms),
            error: Some(format!("timeout after {timeout_secs}s")),
        },
    }
}

/// Run health checks against all (or filtered) ROTATION entries concurrently.
pub async fn run_check_models(
    db: &Option<tokio_postgres::Client>,
    filter_provider: Option<&str>,
    timeout_secs: u64,
) -> Vec<ProbeResult> {
    let entries: Vec<&'static RotationEntry> = ROTATION
        .iter()
        .filter(|e| match filter_provider {
            Some(p) => e.provider == p,
            None => true,
        })
        .collect();

    info!(
        count = entries.len(),
        filter = filter_provider.unwrap_or("(all)"),
        timeout_secs = timeout_secs,
        "Starting model health check"
    );

    // Spawn all probes concurrently.
    let handles: Vec<_> = entries
        .into_iter()
        .map(|entry| tokio::spawn(probe_model(entry, timeout_secs)))
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                warn!("probe task panicked: {e}");
            }
        }
    }

    print_results_table(&results);
    record_health_results(db, &results).await;

    results
}

// ── Output ───────────────────────────────────────────────────────────────────

fn print_results_table(results: &[ProbeResult]) {
    println!(
        "\n  {:<24} {:<14} {:<6} {:>8}  ERROR",
        "MODEL", "PROVIDER", "STATUS", "LATENCY"
    );
    println!("  {}", "-".repeat(90));

    for r in results {
        let latency_str = match r.latency_ms {
            Some(ms) => format!("{ms}ms"),
            None => "-".to_string(),
        };
        let error_str = r.error.as_deref().unwrap_or("");
        // Truncate error for table display.
        let error_display = if error_str.len() > 60 {
            format!("{}…", &error_str[..59])
        } else {
            error_str.to_string()
        };

        println!(
            "  {:<24} {:<14} {:<6} {:>8}  {}",
            r.model_id, r.provider, r.status, latency_str, error_display
        );
    }

    let ok_count = results.iter().filter(|r| r.status == ProbeStatus::Ok).count();
    let fail_count = results.iter().filter(|r| r.status == ProbeStatus::Fail).count();
    let skip_count = results.iter().filter(|r| r.status == ProbeStatus::Skipped).count();

    println!();
    println!(
        "  Total: {} ok, {} failed, {} skipped (out of {})",
        ok_count,
        fail_count,
        skip_count,
        results.len()
    );
    println!();
}

// ── Persistence ──────────────────────────────────────────────────────────────

async fn record_health_results(db: &Option<tokio_postgres::Client>, results: &[ProbeResult]) {
    let Some(db) = db else { return };

    for r in results {
        let status_str = r.status.to_string();
        let result = db
            .execute(
                "INSERT INTO model_health (model_id, provider, status, latency_ms, error)
                 VALUES ($1, $2, $3, $4, $5)",
                &[
                    &r.model_id,
                    &r.provider,
                    &status_str,
                    &r.latency_ms.map(|ms| ms as i64),
                    &r.error,
                ],
            )
            .await;
        if let Err(e) = result {
            warn!(model_id = r.model_id, "Failed to write model_health row: {e}");
        }
    }
}
