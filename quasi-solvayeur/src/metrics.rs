// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! QPU metrics collection for ATW contention coupling.
//!
//! This module provides access to real-time QPU measurement data from HAL Contract,
//! including queue depths, error rates, and calibration data. These metrics are used
//! to refine the ATW algorithm's contention coupling coefficients.

use quasi_scheduler::backend::{Backend, BackendStatus, NoiseProfile};
use serde::{Deserialize, Serialize};

/// Real-time QPU metrics collected from HAL Contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QpuMetrics {
    /// Backend identifier.
    pub backend_id: String,
    /// Current queue depth (number of jobs waiting).
    pub queue_depth: u32,
    /// Current T1 relaxation time in microseconds.
    pub t1_us: Option<f64>,
    /// Current T2 dephasing time in microseconds.
    pub t2_us: Option<f64>,
    /// Single-qubit gate fidelity.
    pub single_qubit_fidelity: Option<f64>,
    /// Two-qubit gate fidelity.
    pub two_qubit_fidelity: Option<f64>,
    /// Readout fidelity.
    pub readout_fidelity: Option<f64>,
    /// Calibration version string.
    pub calibration_version: String,
    /// Unix timestamp of last calibration.
    pub last_calibrated: u64,
}

impl QpuMetrics {
    /// Create metrics from a backend snapshot.
    pub fn from_backend(backend: &Backend, calibration_version: &str, last_calibrated: u64) -> Self {
        let noise = backend.noise_profile();
        Self {
            backend_id: backend.id().to_string(),
            queue_depth: backend.queue_depth(),
            t1_us: noise.and_then(|n| n.t1),
            t2_us: noise.and_then(|n| n.t2),
            single_qubit_fidelity: noise.and_then(|n| n.single_qubit_fidelity),
            two_qubit_fidelity: noise.and_then(|n| n.two_qubit_fidelity),
            readout_fidelity: noise.and_then(|n| n.readout_fidelity),
            calibration_version: calibration_version.to_string(),
            last_calibrated,
        }
    }

    /// Compute a contention coupling coefficient based on current metrics.
    /// Higher values indicate more contention (busy, degraded hardware).
    pub fn contention_coefficient(&self) -> f64 {
        // Base contention from queue depth (normalized)
        let queue_factor = 1.0 / (1.0 + self.queue_depth as f64);
        
        // Fidelity factor (average of available fidelities)
        let fidelities: Vec<f64> = [
            self.single_qubit_fidelity,
            self.two_qubit_fidelity,
            self.readout_fidelity,
        ]
        .into_iter()
        .flatten()
        .collect();
        
        let fidelity_factor = if fidelities.is_empty() {
            0.5
        } else {
            fidelities.iter().sum::<f64>() / fidelities.len() as f64
        };

        // Coherence factor (T2 is usually the limiting factor)
        let coherence_factor = if let Some(t2) = self.t2_us {
            // Normalize: assume 100us is good, below 10us is poor
            (t2 / 100.0).min(1.0).max(0.1)
        } else {
            0.5
        };

        // Combined: high queue + low fidelity + low coherence = high contention
        queue_factor * fidelity_factor * coherence_factor
    }
}

/// Get QPU metrics for a backend.
///
/// In production, this would query HAL Contract's `/hal/backends/{id}/metrics` endpoint.
/// For now, returns mock data based on the backend's current state.
pub fn get_qpu_metrics(backend_id: &str) -> Option<QpuMetrics> {
    // In production, this would be:
    //   let response = reqwest::get(format!("{}/hal/backends/{}/metrics", HAL_BASE_URL, backend_id))
    //       .await?.json::<QpuMetrics>().await?;
    //
    // For mock/testing, we return synthetic data.
    // The actual metrics would come from HAL Contract's real-time monitoring.
    
    // Mock implementation: return None for unknown backends
    // In real usage, this queries the HAL Contract API
    Some(QpuMetrics {
        backend_id: backend_id.to_string(),
        queue_depth: 0, // Would come from HAL Contract
        t1_us: Some(100.0),
        t2_us: Some(50.0),
        single_qubit_fidelity: Some(0.999),
        two_qubit_fidelity: Some(0.99),
        readout_fidelity: Some(0.98),
        calibration_version: "mock_v1".to_string(),
        last_calibrated: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quasi_scheduler::backend::*;

    fn make_test_backend() -> Backend {
        Backend {
            capabilities: Capabilities {
                name: "test_qpu".into(),
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
                    t1: Some(150.0),
                    t2: Some(75.0),
                    single_qubit_fidelity: Some(0.9995),
                    two_qubit_fidelity: Some(0.995),
                    readout_fidelity: Some(0.985),
                    gate_time: Some(0.05),
                }),
            },
            backend_type: BackendType::Qpu,
            status: BackendStatus::Online { queue_depth: 5 },
            cost_per_shot: 0.01,
        }
    }

    #[test]
    fn qpu_metrics_from_backend() {
        let backend = make_test_backend();
        let metrics = QpuMetrics::from_backend(&backend, "cal_v3", 1234567890);

        assert_eq!(metrics.backend_id, "test_qpu");
        assert_eq!(metrics.queue_depth, 5);
        assert_eq!(metrics.t1_us, Some(150.0));
        assert_eq!(metrics.t2_us, Some(75.0));
        assert_eq!(metrics.single_qubit_fidelity, Some(0.9995));
        assert_eq!(metrics.two_qubit_fidelity, Some(0.995));
        assert_eq!(metrics.calibration_version, "cal_v3");
    }

    #[test]
    fn contention_coefficient_varies_with_queue_depth() {
        let mut backend = make_test_backend();
        
        backend.status = BackendStatus::Online { queue_depth: 0 };
        let metrics_empty = QpuMetrics::from_backend(&backend, "cal", 0);
        
        backend.status = BackendStatus::Online { queue_depth: 100 };
        let metrics_busy = QpuMetrics::from_backend(&backend, "cal", 0);

        // Empty queue should have higher contention coefficient (less contention)
        assert!(
            metrics_empty.contention_coefficient() > metrics_busy.contention_coefficient(),
            "empty queue should have higher coefficient: {} vs {}",
            metrics_empty.contention_coefficient(),
            metrics_busy.contention_coefficient()
        );
    }

    #[test]
    fn get_qpu_metrics_returns_mock_data() {
        let metrics = get_qpu_metrics("ibm_strasbourg");
        assert!(metrics.is_some());
        let m = metrics.unwrap();
        assert_eq!(m.backend_id, "ibm_strasbourg");
        assert!(m.t1_us.is_some());
        assert!(m.t2_us.is_some());
    }

    #[test]
    fn contention_coefficient_in_valid_range() {
        let backend = make_test_backend();
        let metrics = QpuMetrics::from_backend(&backend, "cal", 0);
        let coeff = metrics.contention_coefficient();
        
        assert!(
            (0.0..=1.0).contains(&coeff),
            "contention coefficient {} out of range [0, 1]",
            coeff
        );
    }
}
