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
/// 2. Re-read current on-disk state and merge retry counters (take max) to
///    avoid a TOCTOU race between concurrent draft and solve processes that
///    both loaded state at startup but save at different times.
/// 3. Serialise merged state to pretty JSON
/// 4. Write to `STATE_FILE.tmp`
/// 5. Atomic rename `STATE_FILE.tmp` → `STATE_FILE`
/// 6. Release lock (dropped when `_lock_file` goes out of scope)
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

    // Merge: start from caller's state, then promote any higher retry counts
    // that a concurrent process may have written since we last loaded.
    let mut merged = state.clone();
    let path = Path::new(STATE_FILE);
    if path.exists() {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(disk) = serde_json::from_str::<SenateState>(&raw) {
                for (k, v) in &disk.solve_retries {
                    let e = merged.solve_retries.entry(k.clone()).or_insert(0);
                    *e = (*e).max(*v);
                }
                for (k, v) in &disk.draft_retries {
                    let e = merged.draft_retries.entry(k.clone()).or_insert(0);
                    *e = (*e).max(*v);
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&merged).context("save_state: serialise")?;

    fs::write(&tmp_path, &json).context("save_state: write tmp file")?;

    fs::rename(&tmp_path, STATE_FILE).context("save_state: atomic rename")?;

    // Lock is released here when `lock_file` is dropped.
    drop(lock_file);

    info!("save_state: state written to {STATE_FILE}");
    Ok(())
}
