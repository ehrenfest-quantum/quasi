// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! The ATW loop — the Solvayeur quantum kernel.
//!
//! The `Solvayeur` struct maintains the full ATW state across rounds:
//! bias fields, exploration strength, and history. The caller drives
//! the loop by calling `decide()` to get a backend selection, dispatching
//! the workload, and then calling `observe()` with the reward.

use crate::bias::{update_bias, AtwLearning, Reward};
use crate::epoch::{run_epoch_with_config, EpochError, EpochResult, QpuConfig};
use crate::hamiltonian::AtwParams;

/// ATW kernel state.
pub struct Solvayeur {
    /// Current scheduling parameters.
    pub params: AtwParams,
    /// Learning parameters.
    pub learning: AtwLearning,
    /// History of epochs.
    pub history: Vec<EpochRecord>,
    /// Backend names for display.
    pub backend_names: Vec<String>,
    /// QPU configuration for real hardware execution.
    pub qpu_config: QpuConfig,
}

/// Record of one completed ATW epoch.
#[derive(Debug, Clone)]
pub struct EpochRecord {
    /// Epoch number (0-indexed).
    pub epoch: usize,
    /// Which backend was selected.
    pub backend_index: usize,
    /// Name of the selected backend.
    pub backend_name: String,
    /// Scalar reward received.
    pub reward: f64,
    /// Bias fields after this epoch's update.
    pub bias_snapshot: Vec<f64>,
    /// Number of gates in the scheduling circuit.
    pub gate_count: usize,
    /// Whether this epoch ran on QPU.
    pub ran_on_qpu: bool,
}

impl Solvayeur {
    /// Create a new Solvayeur kernel for the given backends (simulated measurement).
    pub fn new(backends: &[quasi_scheduler::backend::Backend]) -> Self {
        let params = AtwParams::from_backends(backends);
        let backend_names: Vec<String> = backends.iter().map(|b| b.id().to_string()).collect();
        Self {
            params,
            learning: AtwLearning::default(),
            history: Vec::new(),
            backend_names,
            qpu_config: QpuConfig::default(),
        }
    }

    /// Create a Solvayeur kernel that uses a real QPU for scheduling decisions.
    pub fn with_qpu(backends: &[quasi_scheduler::backend::Backend], qpu_config: QpuConfig) -> Self {
        let mut s = Self::new(backends);
        s.qpu_config = qpu_config;
        s
    }

    /// Run one ATW round: compile -> measure -> decode -> return decision.
    ///
    /// Does NOT dispatch or observe reward — the caller does that and
    /// then calls `observe()` with the result.
    pub fn decide(&self) -> Result<EpochResult, EpochError> {
        run_epoch_with_config(&self.params, self.history.len(), &self.qpu_config)
    }

    /// Observe the reward from a dispatching decision and update bias.
    pub fn observe(&mut self, epoch_result: &EpochResult, reward: Reward) {
        let epoch_num = self.history.len();

        // Update bias fields (the ATW learning rule)
        self.params.bias = update_bias(
            &self.params.bias,
            &epoch_result.measurement,
            &reward,
            &self.learning,
        );

        // Decay exploration over time (anneal toward exploitation)
        self.params.exploration *= 0.98;
        // Floor at 0.1 to maintain some exploration
        if self.params.exploration < 0.1 {
            self.params.exploration = 0.1;
        }

        // Record
        let backend_name = if epoch_result.backend_index < self.backend_names.len() {
            self.backend_names[epoch_result.backend_index].clone()
        } else {
            format!("backend_{}", epoch_result.backend_index)
        };

        self.history.push(EpochRecord {
            epoch: epoch_num,
            backend_index: epoch_result.backend_index,
            backend_name,
            reward: reward.score(),
            bias_snapshot: self.params.bias.clone(),
            gate_count: epoch_result.gate_count,
            ran_on_qpu: epoch_result.ran_on_qpu,
        });
    }

    /// Current bias fields (for inspection).
    pub fn bias(&self) -> &[f64] {
        &self.params.bias
    }

    /// Current exploration strength.
    pub fn exploration(&self) -> f64 {
        self.params.exploration
    }

    /// Number of completed epochs.
    pub fn epochs(&self) -> usize {
        self.history.len()
    }

    /// Reconfigure the Solvayeur kernel with a new set of backends.
    ///
    /// This allows dynamic addition/removal of backends without restarting the kernel.
    /// Bias fields are preserved for existing qubits and initialized to zero for new ones.
    /// Contention couplings are recalculated for the new qubit count.
    pub fn reconfigure_backends(&mut self, backends: &[quasi_scheduler::backend::Backend]) {
        let new_params = AtwParams::from_backends(backends);
        let new_backend_names: Vec<String> = backends.iter().map(|b| b.id().to_string()).collect();
        
        // Preserve existing bias fields where possible
        let mut new_bias = vec![0.0; new_params.n_qubits];
        let min_qubits = self.params.n_qubits.min(new_params.n_qubits);
        for i in 0..min_qubits {
            new_bias[i] = self.params.bias[i];
        }
        
        self.params.n_qubits = new_params.n_qubits;
        self.params.n_backends = new_params.n_backends;
        self.params.contention = new_params.contention;
        self.params.bias = new_bias;
        self.backend_names = new_backend_names;
    }

    /// Most frequently selected backend over all epochs.
    pub fn preferred_backend(&self) -> Option<&str> {
        if self.history.is_empty() {
            return None;
        }
        let mut counts = std::collections::HashMap::new();
        for record in &self.history {
            *counts.entry(&record.backend_name).or_insert(0usize) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(name, _)| name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quasi_scheduler::backend::*;

    fn make_backends(n: usize) -> Vec<Backend> {
        (0..n)
            .map(|i| Backend {
                capabilities: Capabilities {
                    name: format!("backend_{i}"),
                    num_qubits: 20,
                    gate_set: GateSet {
                        single_qubit: vec!["h".into(), "rz".into()],
                        two_qubit: vec!["cx".into()],
                        three_qubit: vec![],
                        native: vec!["h".into(), "cx".into(), "rz".into()],
                    },
                    topology: Topology::full(20),
                    max_shots: 10_000,
                    is_simulator: false,
                    features: vec![],
                    noise_profile: Some(NoiseProfile {
                        t1: Some(200.0),
                        t2: Some(100.0),
                        single_qubit_fidelity: Some(0.999),
                        two_qubit_fidelity: Some(0.99),
                        readout_fidelity: Some(0.98),
                        gate_time: Some(0.05),
                    }),
                },
                backend_type: BackendType::Qpu,
                status: BackendStatus::Online { queue_depth: 0 },
                cost_per_shot: 0.01,
            })
            .collect()
    }

    fn good_reward() -> Reward {
        Reward {
            fidelity: 0.99,
            latency_ms: 50.0,
            cost: 0.01,
        }
    }

    fn bad_reward() -> Reward {
        Reward {
            fidelity: 0.1,
            latency_ms: 10000.0,
            cost: 100.0,
        }
    }

    #[test]
    fn new_creates_valid_initial_state() {
        let backends = make_backends(4);
        let kernel = Solvayeur::new(&backends);

        assert_eq!(kernel.params.n_qubits, 2);
        assert_eq!(kernel.params.n_backends, 4);
        assert_eq!(kernel.bias(), &[0.0, 0.0]);
        assert_eq!(kernel.epochs(), 0);
        assert!(kernel.preferred_backend().is_none());
    }

    #[test]
    fn decide_returns_valid_epoch_result() {
        let backends = make_backends(4);
        let kernel = Solvayeur::new(&backends);

        let result = kernel.decide().unwrap();
        assert!(result.backend_index < 4);
        assert_eq!(result.measurement.len(), 2);
    }

    #[test]
    fn observe_updates_bias_fields() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        let result = kernel.decide().unwrap();
        let bias_before = kernel.bias().to_vec();

        kernel.observe(&result, good_reward());

        assert_ne!(kernel.bias(), bias_before.as_slice());
        assert_eq!(kernel.epochs(), 1);
    }

    #[test]
    fn multiple_rounds_converge_toward_rewarded_backend() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        // Run 20 rounds, always rewarding the outcome
        for _ in 0..20 {
            let result = kernel.decide().unwrap();
            kernel.observe(&result, good_reward());
        }

        assert_eq!(kernel.epochs(), 20);
        // Bias should have shifted away from zero
        assert!(
            kernel.bias().iter().any(|&b| b.abs() > 0.01),
            "bias should shift after 20 rewarded rounds: {:?}",
            kernel.bias()
        );
    }

    #[test]
    fn preferred_backend_returns_most_selected() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        for _ in 0..10 {
            let result = kernel.decide().unwrap();
            kernel.observe(&result, good_reward());
        }

        let preferred = kernel.preferred_backend();
        assert!(preferred.is_some());
        assert!(preferred.unwrap().starts_with("backend_"));
    }

    #[test]
    fn exploration_decays_over_epochs() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        let initial_exploration = kernel.exploration();

        for _ in 0..10 {
            let result = kernel.decide().unwrap();
            kernel.observe(&result, good_reward());
        }

        assert!(
            kernel.exploration() < initial_exploration,
            "exploration should decay: {} vs {}",
            kernel.exploration(),
            initial_exploration
        );
        // Should not go below floor
        assert!(kernel.exploration() >= 0.1);
    }

    #[test]
    fn exploration_floor_at_minimum() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        // Run many rounds to drive exploration to floor
        for _ in 0..500 {
            let result = kernel.decide().unwrap();
            kernel.observe(&result, good_reward());
        }

        assert!(
            (kernel.exploration() - 0.1).abs() < 0.01,
            "exploration should reach floor: {}",
            kernel.exploration()
        );
    }

    #[test]
    fn atw_roundtrip_bias_shifts_with_reward() {
        let backends = make_backends(4);
        let mut kernel = Solvayeur::new(&backends);

        // First round
        let result1 = kernel.decide().unwrap();
        let bias_before = kernel.bias().to_vec();
        kernel.observe(&result1, good_reward());
        let bias_after_good = kernel.bias().to_vec();

        // Bias should have changed
        assert_ne!(bias_before, bias_after_good);

        // Second round with bad reward
        let result2 = kernel.decide().unwrap();
        kernel.observe(&result2, bad_reward());
        let bias_after_bad = kernel.bias().to_vec();

        // Bias should continue changing
        assert_ne!(bias_after_good, bias_after_bad);
    }
}
