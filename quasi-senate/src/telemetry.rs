// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Telemetry initialisation — JSON tracing to stdout.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialise the global tracing subscriber.
///
/// Format: JSON (machine-parseable).
/// Filter: `RUST_LOG` env var (default: `info`).
/// Timestamps: included via the JSON layer.
pub fn init_telemetry() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().json())
        .init();
}
