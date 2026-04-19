// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Connectivity scoring plugin for the scheduler.
//!
//! Scores backends based on their qubit connectivity. Fully connected backends
//! receive a perfect score of 1.0. Other backends are scored by the ratio of
//! actual edges to the maximum possible edges for the given qubit count.

use crate::backend::Backend;
use crate::plugin::SchedulerPlugin;
use hal_contract::TopologyKind;

/// Scheduler plugin that scores backends by their connectivity.
pub struct ConnectivityScorer;

impl SchedulerPlugin for ConnectivityScorer {
    fn name(&self) -> &str {
        "connectivity"
    }

    fn score(&self, _job: &crate::job::QuantumJob, backend: &Backend) -> f64 {
        let n = backend.qubit_count() as usize;
        if n <= 1 {
            return 1.0;
        }
        // Maximum possible edges in a fully connected graph.
        let max_edges = n * (n - 1) / 2;
        // Determine actual edge count based on topology kind.
        let edge_count = match backend.capabilities.topology.kind {
            TopologyKind::FullyConnected => return 1.0,
            _ => backend.capabilities.topology.edges.len(),
        };
        let ratio = edge_count as f64 / max_edges as f64;
        if ratio > 1.0 { 1.0 } else { ratio }
    }
}
