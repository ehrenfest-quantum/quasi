// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration test for the ConnectivityScorer plugin.

use quasi_scheduler::backend::BackendType;
use quasi_scheduler::job::{JobRequirements, QuantumJob};
use quasi_scheduler::plugins::connectivity::ConnectivityScorer;
use quasi_scheduler::profiles::all_backends;

#[test]
fn connectivity_plugin_scores_backends() {
    let plugin = ConnectivityScorer;
    let backends = all_backends();
    // Dummy job – requirements are irrelevant for connectivity scoring.
    let job = QuantumJob {
        id: "test_job".into(),
        circuit_hash: [0u8; 32],
        requirements: JobRequirements {
            min_qubits: 1,
            required_gates: vec![],
            circuit_depth: 0,
            shot_count: 1,
            noise_budget: None,
            prefers_qpu: false,
            max_cost: None,
        },
        priority: 0,
        submitted_at: 0,
    };

    let mut positive = 0usize;
    for backend in &backends {
        let score = plugin.score(&job, backend);
        assert!(score >= 0.0 && score <= 1.0, "score out of range for {}", backend.id());
        if score > 0.0 {
            positive += 1;
        }
    }
    // Ensure at least five real backends receive a positive score.
    assert!(positive >= 5, "expected at least 5 backends with positive connectivity score, got {}", positive);

    // The simulator backend is fully connected and should score 1.0.
    let simulator = backends
        .iter()
        .find(|b| b.backend_type == BackendType::Simulator)
        .expect("simulator backend not found");
    let sim_score = plugin.score(&job, simulator);
    assert!((sim_score - 1.0).abs() < f64::EPSILON, "simulator should have perfect connectivity score");
}
