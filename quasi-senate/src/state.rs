// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Persistent `SenateState` — read/write with exclusive file locking.

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs;
use std::path::Path;
use tracing::{info, warn};

use crate::types::SenateState;

pub const STATE_FILE: &str = "/home/vops/quasi-senate-state.json";

/// Load `SenateState` from disk.
///
/// Returns `SenateState::default()` when the file does not exist or cannot be
/// parsed (a warning is logged in the latter case).
pub fn load_state() -> Result<SenateState> {
    let path = Path::new(STATE_FILE);

    if !path.exists() {
        return Ok(SenateState::default());
    }

    let raw = fs::read_to_string(path).context("load_state: read file")?;

    match serde_json::from_str::<SenateState>(&raw) {
        Ok(state) => {
            info!("load_state: loaded from {STATE_FILE}");
            Ok(state)
        }
        Err(err) => {
            warn!("load_state: failed to parse {STATE_FILE}: {err} — using default");
            Ok(SenateState::default())
        }
    }
}

/// Persist `SenateState` to disk atomically using an exclusive lock file.
///
/// Write order:
/// 1. Acquire exclusive lock on `STATE_FILE.lock`
/// 2. Serialise to pretty JSON
/// 3. Write to `STATE_FILE.tmp`
/// 4. Atomic rename `STATE_FILE.tmp` → `STATE_FILE`
/// 5. Release lock (dropped when `_lock_file` goes out of scope)
pub fn save_state(state: &SenateState) -> Result<()> {
    let lock_path = format!("{STATE_FILE}.lock");
    let tmp_path = format!("{STATE_FILE}.tmp");

    // Ensure the lock file exists before locking.
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .context("save_state: open lock file")?;

    lock_file
        .lock_exclusive()
        .context("save_state: acquire exclusive lock")?;

    let json = serde_json::to_string_pretty(state).context("save_state: serialise")?;

    fs::write(&tmp_path, &json).context("save_state: write tmp file")?;

    fs::rename(&tmp_path, STATE_FILE).context("save_state: atomic rename")?;

    // Lock is released here when `lock_file` is dropped.
    drop(lock_file);

    info!("save_state: state written to {STATE_FILE}");
    Ok(())
}
