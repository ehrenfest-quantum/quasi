// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Workload profiler interface — the bridge between Huoma and the scheduler.
//!
//! Before the scheduler picks a backend, it asks: "Is this circuit classically
//! simulable?" Huoma answers by profiling the circuit's entanglement structure.
//!
//! If Huoma's ProjectedTTN can handle it efficiently (low bond dimension), the
//! job stays classical. If entanglement grows exponentially, the job routes to QPU.
//! This is the sinC²-routing decision automated.
//!
//! Huoma benchmark (hiq-lab/huoma#14): 1,000,000 qubits in 5.2 seconds.
//! At this scale, almost everything stays classical — QPU is only invoked
//! when there's provable quantum advantage beyond tensor-network reach.
//!
//! ```text
//! Circuit → Profiler (Huoma) → WorkloadProfile
//!                                    │
//!                          ┌─────────┴─────────┐
//!                          ▼                   ▼
//!                    ClassicalOk          NeedsQuantum
//!                    (stay on Huoma)      (route to QPU)
//! ```

use serde::{Deserialize, Serialize};

use crate::backend::Backend;
use crate::job::QuantumJob;
use crate::plugin::SchedulerPlugin;

// ── Workload Profile ─────────────────────────────────────────────────────────

/// Result of profiling a quantum circuit's classical simulability.
///
/// Produced by Huoma's MPS simulator after a shallow trial simulation
/// (e.g., first few layers only) to estimate entanglement growth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadProfile {
    /// Peak bond dimension observed during trial simulation.
    /// Low (<64) means MPS can handle it. High (>1024) means QPU-territory.
    pub peak_bond_dimension: u64,

    /// Maximum bond dimension Huoma can handle before truncation errors dominate.
    /// Typically 2^12 = 4096 for production, 2^8 = 256 for fast profiling.
    pub max_tractable_bond: u64,

    /// Estimated fidelity if simulated classically at max_tractable_bond.
    /// 1.0 = exact. <0.9 = significant truncation error.
    pub estimated_classical_fidelity: f64,

    /// Entanglement growth rate (linear, polynomial, or exponential).
    pub entanglement_class: EntanglementClass,

    /// Estimated wall-clock time for full classical simulation (seconds).
    pub estimated_sim_time_seconds: f64,

    /// sinC² hardness score if available (from Arvak/QOBLIB).
    /// Higher = harder to simulate classically. Range [0.0, 1.0].
    pub sinc2_hardness: Option<f64>,
}

/// Classification of entanglement growth in the circuit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EntanglementClass {
    /// Bond dimension stays constant — trivially simulable (product states).
    Constant,
    /// Bond dimension grows logarithmically — efficiently simulable.
    Logarithmic,
    /// Bond dimension grows polynomially — simulable with effort.
    Polynomial,
    /// Bond dimension grows exponentially — needs QPU.
    Exponential,
}

impl WorkloadProfile {
    /// Whether the circuit is classically simulable at acceptable fidelity.
    pub fn is_classically_tractable(&self) -> bool {
        self.peak_bond_dimension <= self.max_tractable_bond
            && self.estimated_classical_fidelity >= 0.95
    }

    /// Whether the circuit definitively needs quantum hardware.
    pub fn needs_quantum(&self) -> bool {
        self.entanglement_class == EntanglementClass::Exponential
            || self.peak_bond_dimension > self.max_tractable_bond * 4
    }

    /// Confidence that the routing decision is correct (0.0–1.0).
    /// High when the circuit is clearly classical or clearly quantum.
    /// Low in the "gray zone" where MPS barely manages.
    pub fn routing_confidence(&self) -> f64 {
        if self.needs_quantum() {
            // Clearly needs QPU
            let ratio = self.peak_bond_dimension as f64 / self.max_tractable_bond as f64;
            (1.0 - 1.0 / ratio).min(0.99)
        } else if self.is_classically_tractable() {
            // Clearly classical
            self.estimated_classical_fidelity
        } else {
            // Gray zone
            0.5
        }
    }
}

// ── Profiler Trait ───────────────────────────────────────────────────────────

/// Trait for workload profilers.
///
/// Huoma implements this: given a circuit (by hash or content), it runs a
/// quick trial simulation and returns the entanglement profile.
pub trait WorkloadProfiler: Send + Sync {
    /// Profile a circuit's simulability.
    ///
    /// The circuit is identified by its content hash. The profiler may:
    /// - Look up a cached profile (fast path)
    /// - Run a shallow MPS simulation on the first N layers
    /// - Return a conservative estimate if the circuit is too large to profile quickly
    fn profile(&self, circuit_hash: &[u8; 32], circuit_depth: u32, qubit_count: u32) -> WorkloadProfile;
}

// ── Static Profile (for when Huoma isn't available) ──────────────────────────

/// A simple heuristic profiler that estimates simulability from circuit parameters
/// without actually running a simulation. Used as fallback when Huoma is unavailable.
pub struct HeuristicProfiler {
    /// Bond dimension threshold above which we recommend QPU.
    pub bond_threshold: u64,
}

impl Default for HeuristicProfiler {
    fn default() -> Self {
        Self { bond_threshold: 256 }
    }
}

impl WorkloadProfiler for HeuristicProfiler {
    fn profile(&self, _circuit_hash: &[u8; 32], circuit_depth: u32, qubit_count: u32) -> WorkloadProfile {
        // Heuristic: bond dimension grows roughly as 2^(min(depth, qubits/2))
        // This is a conservative estimate — real circuits are often more structured.
        let effective_entangling_depth = (circuit_depth as f64 * 0.3).min(qubit_count as f64 / 2.0);
        let estimated_bond = 2u64.saturating_pow(effective_entangling_depth as u32);

        let entanglement_class = if effective_entangling_depth < 4.0 {
            EntanglementClass::Constant
        } else if effective_entangling_depth < 10.0 {
            EntanglementClass::Logarithmic
        } else if effective_entangling_depth < 20.0 {
            EntanglementClass::Polynomial
        } else {
            EntanglementClass::Exponential
        };

        let estimated_fidelity = if estimated_bond <= self.bond_threshold {
            1.0
        } else {
            (self.bond_threshold as f64 / estimated_bond as f64).sqrt().min(1.0)
        };

        // Rough wall-clock estimate: O(n * chi^3) where chi = bond dimension
        let chi = estimated_bond.min(self.bond_threshold) as f64;
        let estimated_time = qubit_count as f64 * chi.powi(3) * 1e-9;

        WorkloadProfile {
            peak_bond_dimension: estimated_bond,
            max_tractable_bond: self.bond_threshold,
            estimated_classical_fidelity: estimated_fidelity,
            entanglement_class,
            estimated_sim_time_seconds: estimated_time,
            sinc2_hardness: None,
        }
    }
}

// ── Scheduler Plugin ─────────────────────────────────────────────────────────

/// Scheduler plugin that uses workload profiling to route jobs.
///
/// - If the circuit is classically tractable → strongly prefer simulators
/// - If the circuit needs quantum → strongly prefer QPUs
/// - Gray zone → neutral, let other plugins decide
pub struct ProfilerPlugin {
    profiler: Box<dyn WorkloadProfiler>,
}

impl ProfilerPlugin {
    pub fn new(profiler: impl WorkloadProfiler + 'static) -> Self {
        Self {
            profiler: Box::new(profiler),
        }
    }

    /// Create with the default heuristic profiler.
    pub fn heuristic() -> Self {
        Self::new(HeuristicProfiler::default())
    }

    fn get_profile(&self, job: &QuantumJob) -> WorkloadProfile {
        self.profiler.profile(
            &job.circuit_hash,
            job.requirements.circuit_depth,
            job.requirements.min_qubits,
        )
    }
}

impl SchedulerPlugin for ProfilerPlugin {
    fn name(&self) -> &str {
        "workload_profiler"
    }

    fn filter(&self, job: &QuantumJob, backend: &Backend) -> bool {
        let profile = self.get_profile(job);

        // If clearly needs quantum, filter out simulators
        if profile.needs_quantum() && backend.backend_type == crate::backend::BackendType::Simulator {
            return false;
        }

        true
    }

    fn score(&self, job: &QuantumJob, backend: &Backend) -> f64 {
        let profile = self.get_profile(job);

        match (profile.is_classically_tractable(), &backend.backend_type) {
            // Classically tractable + simulator → strong preference
            (true, crate::backend::BackendType::Simulator) => 0.95,
            // Classically tractable + QPU → wasteful, but allowed
            (true, crate::backend::BackendType::Qpu) => 0.2,
            // Needs quantum + QPU → ideal
            (false, crate::backend::BackendType::Qpu) => 0.9,
            // Needs quantum + simulator → only if nothing else
            (false, crate::backend::BackendType::Simulator) => 0.3,
        }
    }

    fn weight(&self) -> f64 {
        3.0 // High weight — routing decision is the most important factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::*;
    use crate::job::*;

    fn test_backend(backend_type: BackendType) -> Backend {
        Backend {
            capabilities: Capabilities {
                name: format!("test_{:?}", backend_type).to_lowercase(),
                num_qubits: 100,
                gate_set: GateSet {
                    single_qubit: vec!["h".into(), "rz".into()],
                    two_qubit: vec!["cx".into()],
                    three_qubit: vec![],
                    native: vec!["h".into(), "cx".into(), "rz".into()],
                },
                topology: Topology::full(100),
                max_shots: 10_000,
                is_simulator: matches!(backend_type, BackendType::Simulator),
                features: vec![],
                noise_profile: Some(NoiseProfile {
                    t1: Some(200.0),
                    t2: Some(100.0),
                    single_qubit_fidelity: Some(0.999),
                    two_qubit_fidelity: Some(0.99),
                    readout_fidelity: Some(0.98),
                    gate_time: None,
                }),
            },
            backend_type,
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: 0.01,
        }
    }

    fn test_job(depth: u32, qubits: u32) -> QuantumJob {
        QuantumJob {
            id: "test".into(),
            circuit_hash: [0u8; 32],
            requirements: JobRequirements {
                min_qubits: qubits,
                required_gates: vec![],
                circuit_depth: depth,
                shot_count: 1000,
                noise_budget: None,
                prefers_qpu: false,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        }
    }

    #[test]
    fn shallow_circuit_is_classically_tractable() {
        let profiler = HeuristicProfiler::default();
        let profile = profiler.profile(&[0u8; 32], 5, 20);
        assert!(profile.is_classically_tractable());
        assert!(!profile.needs_quantum());
        assert_eq!(profile.entanglement_class, EntanglementClass::Constant);
    }

    #[test]
    fn deep_circuit_needs_quantum() {
        let profiler = HeuristicProfiler::default();
        let profile = profiler.profile(&[0u8; 32], 200, 50);
        assert!(!profile.is_classically_tractable());
        assert!(profile.needs_quantum());
        assert_eq!(profile.entanglement_class, EntanglementClass::Exponential);
    }

    #[test]
    fn medium_circuit_in_gray_zone() {
        let profiler = HeuristicProfiler::default();
        let profile = profiler.profile(&[0u8; 32], 30, 20);
        // Should be polynomial/logarithmic — not clearly either way
        assert!(matches!(
            profile.entanglement_class,
            EntanglementClass::Logarithmic | EntanglementClass::Polynomial
        ));
    }

    #[test]
    fn profiler_plugin_prefers_simulator_for_shallow_circuits() {
        let plugin = ProfilerPlugin::heuristic();
        let job = test_job(5, 10); // shallow
        let sim = test_backend(BackendType::Simulator);
        let qpu = test_backend(BackendType::Qpu);

        let sim_score = plugin.score(&job, &sim);
        let qpu_score = plugin.score(&job, &qpu);
        assert!(sim_score > qpu_score, "simulator should score higher for shallow circuit");
    }

    #[test]
    fn profiler_plugin_prefers_qpu_for_deep_circuits() {
        let plugin = ProfilerPlugin::heuristic();
        let job = test_job(200, 50); // deep
        let sim = test_backend(BackendType::Simulator);
        let qpu = test_backend(BackendType::Qpu);

        let qpu_score = plugin.score(&job, &qpu);
        let sim_score = plugin.score(&job, &sim);
        assert!(qpu_score > sim_score, "QPU should score higher for deep circuit");
    }

    #[test]
    fn profiler_plugin_filters_simulator_for_quantum_circuits() {
        let plugin = ProfilerPlugin::heuristic();
        let job = test_job(200, 50); // deep — needs quantum
        let sim = test_backend(BackendType::Simulator);
        let qpu = test_backend(BackendType::Qpu);

        assert!(!plugin.filter(&job, &sim), "simulator should be filtered out");
        assert!(plugin.filter(&job, &qpu), "QPU should pass filter");
    }

    #[test]
    fn profiler_plugin_allows_both_for_shallow_circuits() {
        let plugin = ProfilerPlugin::heuristic();
        let job = test_job(5, 10); // shallow — classically tractable
        let sim = test_backend(BackendType::Simulator);
        let qpu = test_backend(BackendType::Qpu);

        assert!(plugin.filter(&job, &sim), "simulator should pass");
        assert!(plugin.filter(&job, &qpu), "QPU should pass (wasteful but allowed)");
    }

    #[test]
    fn routing_confidence_high_for_clear_cases() {
        let tractable = WorkloadProfile {
            peak_bond_dimension: 4,
            max_tractable_bond: 256,
            estimated_classical_fidelity: 0.999,
            entanglement_class: EntanglementClass::Constant,
            estimated_sim_time_seconds: 0.001,
            sinc2_hardness: None,
        };
        assert!(tractable.routing_confidence() > 0.9);

        let quantum = WorkloadProfile {
            peak_bond_dimension: 10000,
            max_tractable_bond: 256,
            estimated_classical_fidelity: 0.1,
            entanglement_class: EntanglementClass::Exponential,
            estimated_sim_time_seconds: 1e6,
            sinc2_hardness: Some(0.95),
        };
        assert!(quantum.routing_confidence() > 0.9);
    }

    #[test]
    fn routing_confidence_low_in_gray_zone() {
        let gray = WorkloadProfile {
            peak_bond_dimension: 300,
            max_tractable_bond: 256,
            estimated_classical_fidelity: 0.85,
            entanglement_class: EntanglementClass::Polynomial,
            estimated_sim_time_seconds: 60.0,
            sinc2_hardness: None,
        };
        assert!(gray.routing_confidence() < 0.7);
    }

    #[test]
    fn scheduler_with_profiler_routes_correctly() {
        use crate::scheduler::Scheduler;
        use crate::profiles;

        let backends = profiles::all_backends();
        let scheduler = Scheduler::new()
            .with_plugin(ProfilerPlugin::heuristic())
            .with_plugin(crate::plugin::GateSetFilter)
            .with_plugin(crate::plugin::QubitCountFilter);

        // Shallow circuit → should prefer simulator
        let shallow_job = test_job(3, 5);
        let decision = scheduler.schedule(&shallow_job, &backends);
        match decision {
            ScheduleDecision::Assign { backend_id, .. } => {
                assert!(
                    backend_id.contains("simulator") || backend_id.contains("huoma"),
                    "shallow circuit should route to simulator, got: {backend_id}"
                );
            }
            other => panic!("expected Assign, got {:?}", other),
        }

        // Deep circuit → should prefer QPU
        let deep_job = QuantumJob {
            id: "deep".into(),
            circuit_hash: [1u8; 32],
            requirements: JobRequirements {
                min_qubits: 20,
                required_gates: vec!["rz".into(), "ecr".into()],
                circuit_depth: 200,
                shot_count: 1000,
                noise_budget: None,
                prefers_qpu: false,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        };
        let decision = scheduler.schedule(&deep_job, &backends);
        match decision {
            ScheduleDecision::Assign { backend_id, .. } => {
                // Should NOT be a simulator
                let picked = backends.iter().find(|b| b.id() == backend_id).unwrap();
                assert_eq!(
                    picked.backend_type,
                    BackendType::Qpu,
                    "deep circuit should route to QPU, got: {backend_id}"
                );
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }
}
