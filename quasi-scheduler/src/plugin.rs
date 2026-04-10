// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Filter-score-bind plugin system.
//!
//! Scheduling plugins follow a Kubernetes-style pattern: each plugin can
//! hard-filter backends (reject unsuitable ones) and soft-score remaining
//! candidates (rank by preference). The scheduler combines weighted scores
//! to pick the best backend.

use crate::backend::{Backend, BackendType};
use crate::job::QuantumJob;

/// A scheduling plugin that filters and scores backends for a job.
pub trait SchedulerPlugin: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Hard filter: return `false` to exclude this backend.
    fn filter(&self, _job: &QuantumJob, _backend: &Backend) -> bool {
        true
    }

    /// Soft score: return 0.0 (worst) to 1.0 (best).
    fn score(&self, _job: &QuantumJob, _backend: &Backend) -> f64 {
        0.5
    }

    /// Weight for this plugin's score. Higher = more influence.
    fn weight(&self) -> f64 {
        1.0
    }
}

// ── Built-in plugins ────────────────────────────────────────────────────────

/// Filters backends that lack required gates. Scores by native gate coverage.
pub struct GateSetFilter;

impl SchedulerPlugin for GateSetFilter {
    fn name(&self) -> &str {
        "gate-set"
    }

    fn filter(&self, job: &QuantumJob, backend: &Backend) -> bool {
        backend
            .gate_set
            .supports_all(&job.requirements.required_gates)
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        let required = &job.requirements.required_gates;
        if required.is_empty() {
            return 1.0;
        }
        let supported = required
            .iter()
            .filter(|g| backend.gate_set.supports(g))
            .count();
        supported as f64 / required.len() as f64
    }
}

/// Filters backends with too few qubits. Scores by headroom efficiency.
pub struct QubitCountFilter;

impl SchedulerPlugin for QubitCountFilter {
    fn name(&self) -> &str {
        "qubit-count"
    }

    fn filter(&self, job: &QuantumJob, backend: &Backend) -> bool {
        backend.qubit_count >= job.requirements.min_qubits
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        if backend.qubit_count == 0 || job.requirements.min_qubits == 0 {
            return 0.5;
        }
        // Closer to exact match = higher score (avoids wasting qubits).
        let ratio = job.requirements.min_qubits as f64 / backend.qubit_count as f64;
        ratio.min(1.0)
    }
}

/// Filters backends that violate noise constraints. Scores by noise margin.
pub struct NoiseBudgetPlugin;

impl SchedulerPlugin for NoiseBudgetPlugin {
    fn name(&self) -> &str {
        "noise-budget"
    }

    fn filter(&self, job: &QuantumJob, backend: &Backend) -> bool {
        let Some(budget) = &job.requirements.noise_budget else {
            return true;
        };
        if backend.noise.t1_us < budget.min_t1_us {
            return false;
        }
        if backend.noise.t2_us < budget.min_t2_us {
            return false;
        }
        if let Some(max_err) = budget.max_gate_error {
            if backend.noise.two_qubit_error > max_err {
                return false;
            }
        }
        true
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        let Some(budget) = &job.requirements.noise_budget else {
            return 0.5;
        };
        // Score by how much margin above the threshold.
        let t1_margin = if budget.min_t1_us > 0.0 {
            (backend.noise.t1_us / budget.min_t1_us).min(2.0) / 2.0
        } else {
            1.0
        };
        let t2_margin = if budget.min_t2_us > 0.0 {
            (backend.noise.t2_us / budget.min_t2_us).min(2.0) / 2.0
        } else {
            1.0
        };
        (t1_margin + t2_margin) / 2.0
    }
}

/// Scores backends by queue depth (shorter queue = higher score).
pub struct QueueDepthPlugin;

impl SchedulerPlugin for QueueDepthPlugin {
    fn name(&self) -> &str {
        "queue-depth"
    }

    fn score(&self, _job: &QuantumJob, backend: &Backend) -> f64 {
        1.0 / (1.0 + backend.queue_depth() as f64)
    }
}

/// Filters backends that exceed cost budget. Scores by inverse cost.
pub struct CostPlugin;

impl SchedulerPlugin for CostPlugin {
    fn name(&self) -> &str {
        "cost"
    }

    fn filter(&self, job: &QuantumJob, backend: &Backend) -> bool {
        let Some(max_cost) = job.requirements.max_cost else {
            return true;
        };
        backend.cost_per_shot * job.requirements.shot_count as f64 <= max_cost
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        let total_cost =
            backend.cost_per_shot * job.requirements.shot_count as f64;
        if total_cost <= 0.0 {
            return 1.0;
        }
        // Inverse: cheaper is better, capped at 1.0.
        (1.0 / (1.0 + total_cost)).min(1.0)
    }
}

/// Scores backends by type preference (QPU vs simulator).
pub struct BackendTypePlugin;

impl SchedulerPlugin for BackendTypePlugin {
    fn name(&self) -> &str {
        "backend-type"
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        if job.requirements.prefers_qpu {
            if backend.backend_type == BackendType::Qpu {
                1.0
            } else {
                0.5
            }
        } else if backend.backend_type == BackendType::Simulator {
            0.8
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::*;
    use crate::job::*;

    fn make_backend(overrides: impl FnOnce(&mut Backend)) -> Backend {
        let mut b = Backend {
            id: "test-backend".into(),
            name: "Test Backend".into(),
            backend_type: BackendType::Qpu,
            qubit_count: 20,
            gate_set: GateSet {
                native_gates: vec!["h".into(), "cx".into(), "rz".into()],
            },
            topology: Topology::AllToAll,
            noise: NoiseProfile {
                t1_us: 100.0,
                t2_us: 50.0,
                single_qubit_error: 0.001,
                two_qubit_error: 0.01,
                readout_error: 0.02,
                calibration_version: "v1".into(),
            },
            status: BackendStatus::Online { queue_depth: 5 },
            cost_per_shot: 0.01,
        };
        overrides(&mut b);
        b
    }

    fn make_job(overrides: impl FnOnce(&mut QuantumJob)) -> QuantumJob {
        let mut j = QuantumJob {
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
        };
        overrides(&mut j);
        j
    }

    #[test]
    fn gate_set_filter_accepts_matching_backend() {
        let plugin = GateSetFilter;
        let job = make_job(|_| {});
        let backend = make_backend(|_| {});
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn gate_set_filter_rejects_missing_gate() {
        let plugin = GateSetFilter;
        let job = make_job(|j| {
            j.requirements.required_gates = vec!["h".into(), "t".into()];
        });
        let backend = make_backend(|_| {});
        assert!(!plugin.filter(&job, &backend));
    }

    #[test]
    fn gate_set_filter_scores_full_coverage() {
        let plugin = GateSetFilter;
        let job = make_job(|_| {});
        let backend = make_backend(|_| {});
        assert!((plugin.score(&job, &backend) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qubit_count_filter_accepts_enough_qubits() {
        let plugin = QubitCountFilter;
        let job = make_job(|_| {});
        let backend = make_backend(|_| {});
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn qubit_count_filter_rejects_too_few_qubits() {
        let plugin = QubitCountFilter;
        let job = make_job(|j| j.requirements.min_qubits = 50);
        let backend = make_backend(|_| {});
        assert!(!plugin.filter(&job, &backend));
    }

    #[test]
    fn qubit_count_filter_scores_exact_match_highest() {
        let plugin = QubitCountFilter;
        let job = make_job(|j| j.requirements.min_qubits = 20);
        let backend = make_backend(|_| {});
        assert!((plugin.score(&job, &backend) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qubit_count_filter_scores_overprovisioned_lower() {
        let plugin = QubitCountFilter;
        let job = make_job(|j| j.requirements.min_qubits = 5);
        let backend = make_backend(|_| {}); // 20 qubits
        let score = plugin.score(&job, &backend);
        assert!(score < 1.0);
        assert!((score - 0.25).abs() < f64::EPSILON); // 5/20
    }

    #[test]
    fn noise_budget_filter_accepts_when_no_budget() {
        let plugin = NoiseBudgetPlugin;
        let job = make_job(|_| {});
        let backend = make_backend(|_| {});
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn noise_budget_filter_accepts_good_noise() {
        let plugin = NoiseBudgetPlugin;
        let job = make_job(|j| {
            j.requirements.noise_budget = Some(NoiseBudget {
                min_t1_us: 50.0,
                min_t2_us: 30.0,
                max_gate_error: None,
                min_fidelity: None,
            });
        });
        let backend = make_backend(|_| {}); // t1=100, t2=50
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn noise_budget_filter_rejects_bad_t1() {
        let plugin = NoiseBudgetPlugin;
        let job = make_job(|j| {
            j.requirements.noise_budget = Some(NoiseBudget {
                min_t1_us: 200.0,
                min_t2_us: 10.0,
                max_gate_error: None,
                min_fidelity: None,
            });
        });
        let backend = make_backend(|_| {}); // t1=100 < 200
        assert!(!plugin.filter(&job, &backend));
    }

    #[test]
    fn noise_budget_filter_rejects_bad_gate_error() {
        let plugin = NoiseBudgetPlugin;
        let job = make_job(|j| {
            j.requirements.noise_budget = Some(NoiseBudget {
                min_t1_us: 10.0,
                min_t2_us: 10.0,
                max_gate_error: Some(0.005),
                min_fidelity: None,
            });
        });
        let backend = make_backend(|_| {}); // two_qubit_error=0.01 > 0.005
        assert!(!plugin.filter(&job, &backend));
    }

    #[test]
    fn queue_depth_shorter_queue_scores_higher() {
        let plugin = QueueDepthPlugin;
        let job = make_job(|_| {});
        let short_q = make_backend(|b| {
            b.status = BackendStatus::Online { queue_depth: 1 };
        });
        let long_q = make_backend(|b| {
            b.status = BackendStatus::Online { queue_depth: 100 };
        });
        assert!(plugin.score(&job, &short_q) > plugin.score(&job, &long_q));
    }

    #[test]
    fn cost_filter_accepts_within_budget() {
        let plugin = CostPlugin;
        let job = make_job(|j| j.requirements.max_cost = Some(100.0));
        let backend = make_backend(|_| {}); // 0.01 * 1000 = 10.0 <= 100.0
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn cost_filter_rejects_over_budget() {
        let plugin = CostPlugin;
        let job = make_job(|j| {
            j.requirements.max_cost = Some(5.0);
        });
        let backend = make_backend(|b| b.cost_per_shot = 1.0); // 1.0 * 1000 = 1000 > 5
        assert!(!plugin.filter(&job, &backend));
    }

    #[test]
    fn cost_filter_accepts_when_no_budget() {
        let plugin = CostPlugin;
        let job = make_job(|_| {});
        let backend = make_backend(|b| b.cost_per_shot = 999.0);
        assert!(plugin.filter(&job, &backend));
    }

    #[test]
    fn backend_type_prefers_qpu_when_requested() {
        let plugin = BackendTypePlugin;
        let job = make_job(|j| j.requirements.prefers_qpu = true);
        let qpu = make_backend(|b| b.backend_type = BackendType::Qpu);
        let sim = make_backend(|b| b.backend_type = BackendType::Simulator);
        assert!(plugin.score(&job, &qpu) > plugin.score(&job, &sim));
    }

    #[test]
    fn backend_type_prefers_simulator_when_not_requesting_qpu() {
        let plugin = BackendTypePlugin;
        let job = make_job(|j| j.requirements.prefers_qpu = false);
        let qpu = make_backend(|b| b.backend_type = BackendType::Qpu);
        let sim = make_backend(|b| b.backend_type = BackendType::Simulator);
        assert!(plugin.score(&job, &sim) > plugin.score(&job, &qpu));
    }
}
