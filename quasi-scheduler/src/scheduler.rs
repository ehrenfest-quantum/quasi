// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! The resource scheduler.
//!
//! Combines filter-score-bind plugins to select the best backend for a
//! quantum job from a pool of available backends.

use tracing::info;

use crate::backend::Backend;
use crate::job::{QuantumJob, ScheduleDecision};
use crate::plugin::{
    BackendTypePlugin, CostPlugin, GateSetFilter, NoiseBudgetPlugin, QueueDepthPlugin,
    QubitCountFilter, SchedulerPlugin,
};

/// The resource scheduler.
pub struct Scheduler {
    plugins: Vec<Box<dyn SchedulerPlugin>>,
}

impl Scheduler {
    /// Create an empty scheduler with no plugins.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Add a scheduling plugin (builder pattern).
    pub fn with_plugin(mut self, plugin: impl SchedulerPlugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Create a scheduler with all built-in plugins.
    pub fn with_default_plugins() -> Self {
        Self::new()
            .with_plugin(GateSetFilter)
            .with_plugin(QubitCountFilter)
            .with_plugin(NoiseBudgetPlugin)
            .with_plugin(QueueDepthPlugin)
            .with_plugin(CostPlugin)
            .with_plugin(BackendTypePlugin)
    }

    /// Schedule a job across available backends.
    ///
    /// 1. **Filter**: keep only online backends that pass all plugins' filters.
    /// 2. **Score**: compute weighted sum of all plugin scores.
    /// 3. **Bind**: pick the backend with the highest score.
    pub fn schedule(&self, job: &QuantumJob, backends: &[Backend]) -> ScheduleDecision {
        // 1. Filter: keep only backends that pass ALL plugins' filters.
        let candidates: Vec<&Backend> = backends
            .iter()
            .filter(|b| b.is_online())
            .filter(|b| self.plugins.iter().all(|p| p.filter(job, b)))
            .collect();

        if candidates.is_empty() {
            info!(
                job_id = %job.id,
                "no backends pass all filter constraints"
            );
            return ScheduleDecision::Rejected {
                reason: "No backends pass all filter constraints".into(),
            };
        }

        // 2. Score: weighted sum of all plugin scores.
        let total_weight: f64 = self.plugins.iter().map(|p| p.weight()).sum();
        let divisor = total_weight.max(1.0);

        let scored: Vec<(&Backend, f64)> = candidates
            .iter()
            .map(|b| {
                let weighted_score: f64 = self
                    .plugins
                    .iter()
                    .map(|p| p.score(job, b) * p.weight())
                    .sum::<f64>()
                    / divisor;
                (*b, weighted_score)
            })
            .collect();

        // 3. Bind: pick highest score.
        let (best_backend, best_score) = scored
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .expect("candidates is non-empty");

        info!(
            job_id = %job.id,
            backend_id = %best_backend.id(),
            score = best_score,
            "scheduled job"
        );

        ScheduleDecision::Assign {
            backend_id: best_backend.id().to_string(),
            score: best_score,
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::*;
    use crate::job::*;

    fn make_backend_with(
        id: &str,
        backend_type: BackendType,
        qubit_count: u32,
        gates: &[&str],
        queue_depth: u32,
        cost: f64,
        t1: f64,
        t2: f64,
    ) -> Backend {
        let single_qubit: Vec<String> = gates.iter().map(|s| s.to_string()).collect();
        let two_qubit: Vec<String> = gates.iter().map(|s| s.to_string()).collect();
        Backend {
            capabilities: Capabilities {
                name: id.into(),
                num_qubits: qubit_count,
                gate_set: GateSet {
                    single_qubit: single_qubit.clone(),
                    two_qubit,
                    three_qubit: vec![],
                    native: single_qubit,
                },
                topology: Topology::full(qubit_count),
                max_shots: 10_000,
                is_simulator: matches!(backend_type, BackendType::Simulator),
                features: vec![],
                noise_profile: Some(NoiseProfile {
                    t1: Some(t1),
                    t2: Some(t2),
                    single_qubit_fidelity: Some(0.999),
                    two_qubit_fidelity: Some(0.99),
                    readout_fidelity: Some(0.98),
                    gate_time: None,
                }),
            },
            backend_type,
            status: BackendStatus::Online { queue_depth },
            cost_per_shot: cost,
        }
    }

    fn make_job_default() -> QuantumJob {
        QuantumJob {
            id: "job-1".into(),
            circuit_hash: [0u8; 32],
            requirements: JobRequirements {
                min_qubits: 5,
                required_gates: vec!["h".into(), "cx".into()],
                circuit_depth: 10,
                shot_count: 1000,
                noise_budget: None,
                prefers_qpu: true,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        }
    }

    #[test]
    fn scheduler_picks_best_backend() {
        let scheduler = Scheduler::with_default_plugins();
        let job = make_job_default();

        let backends = vec![
            // Good QPU: right gates, short queue, cheap.
            make_backend_with(
                "good-qpu",
                BackendType::Qpu,
                10,
                &["h", "cx", "rz"],
                2,
                0.001,
                100.0,
                50.0,
            ),
            // Worse QPU: long queue, expensive.
            make_backend_with(
                "bad-qpu",
                BackendType::Qpu,
                50,
                &["h", "cx", "rz"],
                100,
                0.1,
                100.0,
                50.0,
            ),
        ];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Assign { backend_id, score } => {
                assert_eq!(backend_id, "good-qpu");
                assert!(score > 0.0);
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_rejects_when_no_eligible_backends() {
        let scheduler = Scheduler::with_default_plugins();
        let job = make_job_default();

        // All backends missing required gate "cx".
        let backends = vec![make_backend_with(
            "no-cx",
            BackendType::Qpu,
            10,
            &["h", "rz"],
            2,
            0.001,
            100.0,
            50.0,
        )];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Rejected { reason } => {
                assert!(reason.contains("No backends"));
            }
            other => panic!("expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_rejects_when_all_offline() {
        let scheduler = Scheduler::with_default_plugins();
        let job = make_job_default();

        let mut backend = make_backend_with(
            "offline",
            BackendType::Qpu,
            10,
            &["h", "cx"],
            0,
            0.001,
            100.0,
            50.0,
        );
        backend.status = BackendStatus::Offline;

        match scheduler.schedule(&job, &[backend]) {
            ScheduleDecision::Rejected { .. } => {}
            other => panic!("expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_filters_by_noise_budget() {
        let scheduler = Scheduler::with_default_plugins();
        let mut job = make_job_default();
        job.requirements.noise_budget = Some(NoiseBudget {
            min_t1_us: 80.0,
            min_t2_us: 40.0,
            max_gate_error: None,
            min_fidelity: None,
        });

        let backends = vec![
            // Good noise.
            make_backend_with(
                "good-noise",
                BackendType::Qpu,
                10,
                &["h", "cx"],
                5,
                0.01,
                100.0,
                50.0,
            ),
            // Bad noise: t1 too low.
            make_backend_with(
                "bad-noise",
                BackendType::Qpu,
                10,
                &["h", "cx"],
                5,
                0.01,
                50.0,
                50.0,
            ),
        ];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "good-noise");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_filters_by_cost() {
        let scheduler = Scheduler::with_default_plugins();
        let mut job = make_job_default();
        job.requirements.max_cost = Some(5.0); // 1000 shots * cost <= 5.0

        let backends = vec![
            // Cheap: 0.001 * 1000 = 1.0.
            make_backend_with(
                "cheap",
                BackendType::Qpu,
                10,
                &["h", "cx"],
                5,
                0.001,
                100.0,
                50.0,
            ),
            // Expensive: 1.0 * 1000 = 1000.0.
            make_backend_with(
                "expensive",
                BackendType::Qpu,
                10,
                &["h", "cx"],
                5,
                1.0,
                100.0,
                50.0,
            ),
        ];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "cheap");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_prefers_simulator_when_not_preferring_qpu() {
        let scheduler = Scheduler::with_default_plugins();
        let mut job = make_job_default();
        job.requirements.prefers_qpu = false;

        let backends = vec![
            // QPU.
            make_backend_with(
                "qpu",
                BackendType::Qpu,
                10,
                &["h", "cx"],
                5,
                0.01,
                100.0,
                50.0,
            ),
            // Simulator: same everything else, but type differs.
            make_backend_with(
                "sim",
                BackendType::Simulator,
                10,
                &["h", "cx"],
                5,
                0.01,
                100.0,
                50.0,
            ),
        ];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "sim");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_with_no_plugins_accepts_any_online_backend() {
        let scheduler = Scheduler::new();
        let job = make_job_default();

        let backends = vec![make_backend_with(
            "any",
            BackendType::Simulator,
            1,
            &[],
            0,
            0.0,
            10.0,
            5.0,
        )];

        match scheduler.schedule(&job, &backends) {
            ScheduleDecision::Assign { backend_id, .. } => {
                assert_eq!(backend_id, "any");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn scheduler_maintenance_backend_filtered_out() {
        let scheduler = Scheduler::with_default_plugins();
        let job = make_job_default();

        let mut backend = make_backend_with(
            "maint",
            BackendType::Qpu,
            10,
            &["h", "cx"],
            0,
            0.001,
            100.0,
            50.0,
        );
        backend.status = BackendStatus::Maintenance;

        match scheduler.schedule(&job, &[backend]) {
            ScheduleDecision::Rejected { .. } => {}
            other => panic!("expected Rejected, got {:?}", other),
        }
    }
}
