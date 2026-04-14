// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Per-qubit calibration data and region scoring.
//!
//! Extends HAL Contract's device-wide `NoiseProfile` with per-qubit detail.
//! Calibration data flows from `GET /hal/backends/{name}/calibration` at
//! runtime; the `BackendCalibration::mock()` constructor generates realistic
//! test data with quality gradients and deterministic defects.

use serde::{Deserialize, Serialize};

/// Per-qubit calibration snapshot from the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QubitCalibration {
    /// T1 relaxation time for this qubit (microseconds).
    pub t1: f64,
    /// T2 dephasing time for this qubit (microseconds).
    pub t2: f64,
    /// Single-qubit gate fidelity for this qubit.
    pub gate_fidelity: f64,
    /// Readout fidelity for this qubit.
    pub readout_fidelity: f64,
}

/// Per-edge (two-qubit gate) calibration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCalibration {
    /// First qubit of the edge.
    pub qubit_a: u32,
    /// Second qubit of the edge.
    pub qubit_b: u32,
    /// Two-qubit gate fidelity on this edge.
    pub gate_fidelity: f64,
}

/// Full calibration snapshot for a backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCalibration {
    /// Backend identifier.
    pub backend_name: String,
    /// Unix timestamp of the calibration snapshot.
    pub timestamp: u64,
    /// Per-qubit calibration data, indexed by physical qubit index.
    pub qubits: Vec<QubitCalibration>,
    /// Per-edge calibration data.
    pub edges: Vec<EdgeCalibration>,
}

impl BackendCalibration {
    /// Score a single qubit: higher = better quality.
    ///
    /// Weighted combination:
    /// - 40% gate fidelity (dominates circuit quality)
    /// - 30% T2 coherence (limits circuit depth)
    /// - 20% readout fidelity (limits measurement accuracy)
    /// - 10% T1 relaxation (secondary coherence bound)
    ///
    /// T1 and T2 are normalised against reference values (300μs and 200μs)
    /// and clamped to [0, 1].
    pub fn qubit_score(&self, qubit: u32) -> f64 {
        let q = &self.qubits[qubit as usize];
        0.4 * q.gate_fidelity
            + 0.3 * (q.t2 / 200.0).min(1.0)
            + 0.2 * q.readout_fidelity
            + 0.1 * (q.t1 / 300.0).min(1.0)
    }

    /// Score a contiguous region of qubits: qubit quality + connectivity.
    ///
    /// Returns 0.0 for an empty region. The score blends:
    /// - 60% average per-qubit quality
    /// - 40% average two-qubit gate fidelity on edges within the region
    ///   (defaults to 0.5 when no internal edges exist)
    pub fn region_score(&self, qubits: &[u32]) -> f64 {
        if qubits.is_empty() {
            return 0.0;
        }

        let qubit_avg: f64 =
            qubits.iter().map(|&q| self.qubit_score(q)).sum::<f64>() / qubits.len() as f64;

        let mut edge_score = 0.0;
        let mut edge_count = 0u32;
        for i in 0..qubits.len() {
            for j in (i + 1)..qubits.len() {
                if let Some(edge) = self.edges.iter().find(|e| {
                    (e.qubit_a == qubits[i] && e.qubit_b == qubits[j])
                        || (e.qubit_a == qubits[j] && e.qubit_b == qubits[i])
                }) {
                    edge_score += edge.gate_fidelity;
                    edge_count += 1;
                }
            }
        }
        let edge_avg = if edge_count > 0 {
            edge_score / edge_count as f64
        } else {
            0.5
        };

        0.6 * qubit_avg + 0.4 * edge_avg
    }

    /// Generate a mock calibration for testing, with realistic variation.
    ///
    /// Quality gradient: centre qubits are better (common in real hardware).
    /// Deterministic defects on every 17th qubit (simulates TLS defects).
    pub fn mock(backend_name: &str, n_qubits: u32, edges: &[(u32, u32)]) -> Self {
        let qubits: Vec<QubitCalibration> = (0..n_qubits)
            .map(|q| {
                let center_dist = ((q as f64) - (n_qubits as f64 / 2.0)).abs()
                    / (n_qubits as f64 / 2.0);
                let quality = 1.0 - 0.15 * center_dist;
                let defect = if q % 17 == 0 { 0.2 } else { 0.0 };
                QubitCalibration {
                    t1: 200.0 * quality - 40.0 * defect,
                    t2: 150.0 * quality - 30.0 * defect,
                    gate_fidelity: (0.999 * quality - 0.05 * defect).max(0.9),
                    readout_fidelity: (0.98 * quality - 0.03 * defect).max(0.9),
                }
            })
            .collect();

        let edge_cals: Vec<EdgeCalibration> = edges
            .iter()
            .map(|&(a, b)| {
                let avg_quality = (qubits[a as usize].gate_fidelity
                    + qubits[b as usize].gate_fidelity)
                    / 2.0;
                EdgeCalibration {
                    qubit_a: a,
                    qubit_b: b,
                    gate_fidelity: avg_quality * 0.995,
                }
            })
            .collect();

        BackendCalibration {
            backend_name: backend_name.into(),
            timestamp: 0,
            qubits,
            edges: edge_cals,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_edges(n: u32) -> Vec<(u32, u32)> {
        (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
    }

    #[test]
    fn mock_creates_correct_number_of_qubits_and_edges() {
        let edges = linear_edges(20);
        let cal = BackendCalibration::mock("test_backend", 20, &edges);
        assert_eq!(cal.qubits.len(), 20);
        assert_eq!(cal.edges.len(), 19);
        assert_eq!(cal.backend_name, "test_backend");
    }

    #[test]
    fn qubit_score_is_between_zero_and_one() {
        let edges = linear_edges(50);
        let cal = BackendCalibration::mock("test", 50, &edges);
        for q in 0..50 {
            let score = cal.qubit_score(q);
            assert!(
                (0.0..=1.0).contains(&score),
                "qubit {q} score {score} out of range"
            );
        }
    }

    #[test]
    fn defective_qubits_score_lower() {
        let edges = linear_edges(50);
        let cal = BackendCalibration::mock("test", 50, &edges);

        // Qubit 0 and 17 and 34 are defective (q % 17 == 0)
        // Compare defective qubit 17 with its neighbour 18
        let score_defective = cal.qubit_score(17);
        let score_good = cal.qubit_score(18);
        assert!(
            score_defective < score_good,
            "defective qubit 17 ({score_defective}) should score lower than qubit 18 ({score_good})"
        );
    }

    #[test]
    fn centre_qubits_score_higher_than_edge_qubits() {
        let edges = linear_edges(50);
        let cal = BackendCalibration::mock("test", 50, &edges);

        // Centre qubit (25) vs edge qubit (1, avoiding defective 0)
        let centre = cal.qubit_score(25);
        let edge = cal.qubit_score(1);
        assert!(
            centre > edge,
            "centre qubit ({centre}) should score higher than edge qubit ({edge})"
        );
    }

    #[test]
    fn region_score_empty_is_zero() {
        let cal = BackendCalibration::mock("test", 10, &linear_edges(10));
        assert_eq!(cal.region_score(&[]), 0.0);
    }

    #[test]
    fn region_score_good_region_beats_bad_region() {
        let edges = linear_edges(50);
        let cal = BackendCalibration::mock("test", 50, &edges);

        // Good region: centre qubits (avoiding defective 34)
        let good_region = vec![23, 24, 25, 26, 27];
        // Bad region: edge qubits including defective qubit 0
        let bad_region = vec![0, 1, 2, 3, 4];

        let good_score = cal.region_score(&good_region);
        let bad_score = cal.region_score(&bad_region);
        assert!(
            good_score > bad_score,
            "good region ({good_score}) should beat bad region ({bad_score})"
        );
    }

    #[test]
    fn calibration_roundtrip_serde() {
        let cal = BackendCalibration::mock("serde_test", 5, &[(0, 1), (1, 2)]);
        let json = serde_json::to_string(&cal).unwrap();
        let restored: BackendCalibration = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.backend_name, "serde_test");
        assert_eq!(restored.qubits.len(), 5);
        assert_eq!(restored.edges.len(), 2);
    }
}
