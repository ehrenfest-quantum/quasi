// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Backend health monitoring plugin.
//!
//! Provides a simple health fetcher that reads the backend's current status
//! (online/offline and queue depth) and exposes a `SchedulerPlugin` that
//! filters out offline backends and scores based on queue depth.
//! The plugin can be extended to query the HAL Contract for live metrics.

use crate::backend::{Backend, BackendStatus};
use crate::plugin::SchedulerPlugin;

/// Health information for a backend.
#[derive(Debug, Clone, Copy)]
pub struct BackendHealth {
    /// Whether the backend is online.
    pub is_online: bool,
    /// Current queue depth (number of pending jobs).
    pub queue_depth: u32,
}

/// Trait for fetching health data for a backend.
///
/// In production this could perform an HTTP request to the HAL Contract
/// service. The default implementation simply reads the `Backend` fields.
pub trait HealthFetcher: Send + Sync {
    fn fetch(&self, backend: &Backend) -> BackendHealth;
}

/// Simple fetcher that extracts health from the `Backend` struct.
pub struct SimpleHealthFetcher;

impl HealthFetcher for SimpleHealthFetcher {
    fn fetch(&self, backend: &Backend) -> BackendHealth {
        let (is_online, queue_depth) = match &backend.status {
            BackendStatus::Online { queue_depth } => (true, *queue_depth),
            _ => (false, u32::MAX),
        };
        BackendHealth { is_online, queue_depth }
    }
}

/// Scheduler plugin that uses health data to filter and score backends.
pub struct HealthMonitorPlugin {
    fetcher: Box<dyn HealthFetcher>,
}

impl HealthMonitorPlugin {
    /// Create a new plugin with the default simple fetcher.
    pub fn new() -> Self {
        Self {
            fetcher: Box::new(SimpleHealthFetcher),
        }
    }

    /// Create a plugin with a custom fetcher (useful for testing).
    pub fn with_fetcher(fetcher: impl HealthFetcher + 'static) -> Self {
        Self {
            fetcher: Box::new(fetcher),
        }
    }
}

impl SchedulerPlugin for HealthMonitorPlugin {
    fn name(&self) -> &str {
        "health-monitor"
    }

    fn filter(&self, _job: &crate::job::QuantumJob, backend: &Backend) -> bool {
        let health = self.fetcher.fetch(backend);
        health.is_online
    }

    fn score(&self, _job: &crate::job::QuantumJob, backend: &Backend) -> f64 {
        let health = self.fetcher.fetch(backend);
        // Higher score for lower queue depth.
        if health.queue_depth == u32::MAX {
            0.0
        } else {
            1.0 / (1.0 + health.queue_depth as f64)
        }
    }

    fn weight(&self) -> f64 {
        // Give this plugin a strong influence.
        5.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Backend, BackendType, BackendStatus, Capabilities, GateSet, NoiseProfile, Topology};
    use crate::scheduler::Scheduler;
    use crate::job::{QuantumJob, JobRequirements};

    fn make_backend(name: &str, queue: u32, online: bool) -> Backend {
        let status = if online {
            BackendStatus::Online { queue_depth: queue }
        } else {
            BackendStatus::Offline
        };
        Backend {
            capabilities: Capabilities {
                name: name.into(),
                num_qubits: 10,
                gate_set: GateSet {
                    single_qubit: vec![],
                    two_qubit: vec![],
                    three_qubit: vec![],
                    native: vec![],
                },
                topology: Topology::full(10),
                max_shots: 10_000,
                is_simulator: false,
                features: vec![],
                noise_profile: Some(NoiseProfile {
                    t1: Some(100.0),
                    t2: Some(50.0),
                    single_qubit_fidelity: Some(0.999),
                    two_qubit_fidelity: Some(0.99),
                    readout_fidelity: Some(0.98),
                    gate_time: None,
                }),
            },
            backend_type: BackendType::Qpu,
            status,
            cost_per_shot: 0.01,
        }
    }

    #[test]
    fn health_monitor_plugin_prefers_low_queue_backend() {
        // Five backends with varying queue depths.
        let backends = vec![
            make_backend("b0", 10, true),
            make_backend("b1", 5, true),
            make_backend("b2", 0, true), // best
            make_backend("b3", 20, true),
            make_backend("b4", 1, true),
        ];

        let scheduler = Scheduler::new()
            .with_plugin(HealthMonitorPlugin::new())
            .with_plugin(crate::plugin::GateSetFilter); // trivial filter

        let job = QuantumJob {
            id: "test".into(),
            circuit_hash: [0u8; 32],
            requirements: JobRequirements {
                min_qubits: 1,
                required_gates: vec![],
                circuit_depth: 1,
                shot_count: 100,
                noise_budget: None,
                prefers_qpu: true,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        };

        let decision = scheduler.schedule(&job, &backends);
        match decision {
            crate::job::ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "b2");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn health_monitor_filters_offline_backends() {
        let backends = vec![
            make_backend("online", 3, true),
            make_backend("offline", 0, false),
        ];

        let scheduler = Scheduler::new().with_plugin(HealthMonitorPlugin::new());

        let job = QuantumJob {
            id: "test2".into(),
            circuit_hash: [0u8; 32],
            requirements: JobRequirements {
                min_qubits: 1,
                required_gates: vec![],
                circuit_depth: 1,
                shot_count: 100,
                noise_budget: None,
                prefers_qpu: true,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        };

        let decision = scheduler.schedule(&job, &backends);
        match decision {
            crate::job::ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "online");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }
}
