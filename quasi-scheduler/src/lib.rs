// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Heterogeneous resource scheduler for QUASI.
//!
//! Decides which backend (QPU or simulator) should run a given quantum circuit,
//! using a Kubernetes-style filter-score-bind plugin pipeline.

pub mod backend;
pub mod job;
pub mod plugin;
pub mod scheduler;
