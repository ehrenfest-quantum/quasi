// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Backend capability model.
//!
//! Describes the capabilities, noise characteristics, and status of quantum
//! backends available for scheduling.

use serde::{Deserialize, Serialize};

/// Unique backend identifier.
pub type BackendId = String;

/// Gate set supported by a backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateSet {
    pub native_gates: Vec<String>,
}

impl GateSet {
    /// Whether this gate set includes a specific gate.
    pub fn supports(&self, gate: &str) -> bool {
        self.native_gates.iter().any(|g| g == gate)
    }

    /// Whether this gate set includes all of the given gates.
    pub fn supports_all(&self, gates: &[String]) -> bool {
        gates.iter().all(|g| self.supports(g))
    }
}

/// Qubit connectivity topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Topology {
    /// All-to-all connectivity.
    AllToAll,
    /// Linear chain.
    Linear,
    /// Grid with dimensions.
    Grid { rows: usize, cols: usize },
    /// Heavy-hex (IBM style).
    HeavyHex,
    /// Custom adjacency list.
    Custom { edges: Vec<(usize, usize)> },
}

/// Noise characteristics of a backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseProfile {
    /// T1 relaxation time in microseconds.
    pub t1_us: f64,
    /// T2 dephasing time in microseconds.
    pub t2_us: f64,
    /// Single-qubit gate error rate.
    pub single_qubit_error: f64,
    /// Two-qubit gate error rate.
    pub two_qubit_error: f64,
    /// Readout error rate.
    pub readout_error: f64,
    /// Calibration version string.
    pub calibration_version: String,
}

/// Current availability status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendStatus {
    /// Backend is online with a queue depth.
    Online { queue_depth: u32 },
    /// Backend is under maintenance.
    Maintenance,
    /// Backend is offline.
    Offline,
}

/// Hardware type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendType {
    /// Real quantum processing unit.
    Qpu,
    /// Classical simulator.
    Simulator,
}

/// A registered backend with its capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backend {
    pub id: BackendId,
    pub name: String,
    pub backend_type: BackendType,
    pub qubit_count: u32,
    pub gate_set: GateSet,
    pub topology: Topology,
    pub noise: NoiseProfile,
    pub status: BackendStatus,
    pub cost_per_shot: f64,
}

impl Backend {
    /// Whether the backend is currently online.
    pub fn is_online(&self) -> bool {
        matches!(self.status, BackendStatus::Online { .. })
    }

    /// Current queue depth, or `u32::MAX` if not online.
    pub fn queue_depth(&self) -> u32 {
        match &self.status {
            BackendStatus::Online { queue_depth } => *queue_depth,
            _ => u32::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gate_set(gates: &[&str]) -> GateSet {
        GateSet {
            native_gates: gates.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn gate_set_supports_present_gate() {
        let gs = make_gate_set(&["h", "cx", "rz"]);
        assert!(gs.supports("h"));
        assert!(gs.supports("cx"));
        assert!(!gs.supports("t"));
    }

    #[test]
    fn gate_set_supports_all_present_gates() {
        let gs = make_gate_set(&["h", "cx", "rz"]);
        let required: Vec<String> =
            vec!["h".into(), "cx".into()];
        assert!(gs.supports_all(&required));
    }

    #[test]
    fn gate_set_supports_all_rejects_missing_gate() {
        let gs = make_gate_set(&["h", "cx"]);
        let required: Vec<String> =
            vec!["h".into(), "t".into()];
        assert!(!gs.supports_all(&required));
    }

    #[test]
    fn gate_set_supports_all_empty_required() {
        let gs = make_gate_set(&["h"]);
        assert!(gs.supports_all(&[]));
    }

    #[test]
    fn backend_is_online_when_online() {
        let b = Backend {
            id: "b1".into(),
            name: "Test".into(),
            backend_type: BackendType::Qpu,
            qubit_count: 5,
            gate_set: make_gate_set(&[]),
            topology: Topology::AllToAll,
            noise: NoiseProfile {
                t1_us: 100.0,
                t2_us: 50.0,
                single_qubit_error: 0.001,
                two_qubit_error: 0.01,
                readout_error: 0.02,
                calibration_version: "v1".into(),
            },
            status: BackendStatus::Online { queue_depth: 3 },
            cost_per_shot: 0.01,
        };
        assert!(b.is_online());
        assert_eq!(b.queue_depth(), 3);
    }

    #[test]
    fn backend_not_online_when_maintenance() {
        let b = Backend {
            id: "b2".into(),
            name: "Down".into(),
            backend_type: BackendType::Simulator,
            qubit_count: 20,
            gate_set: make_gate_set(&[]),
            topology: Topology::Linear,
            noise: NoiseProfile {
                t1_us: 100.0,
                t2_us: 50.0,
                single_qubit_error: 0.001,
                two_qubit_error: 0.01,
                readout_error: 0.02,
                calibration_version: "v1".into(),
            },
            status: BackendStatus::Maintenance,
            cost_per_shot: 0.0,
        };
        assert!(!b.is_online());
        assert_eq!(b.queue_depth(), u32::MAX);
    }

    #[test]
    fn backend_not_online_when_offline() {
        let b = Backend {
            id: "b3".into(),
            name: "Off".into(),
            backend_type: BackendType::Qpu,
            qubit_count: 10,
            gate_set: make_gate_set(&[]),
            topology: Topology::HeavyHex,
            noise: NoiseProfile {
                t1_us: 100.0,
                t2_us: 50.0,
                single_qubit_error: 0.001,
                two_qubit_error: 0.01,
                readout_error: 0.02,
                calibration_version: "v1".into(),
            },
            status: BackendStatus::Offline,
            cost_per_shot: 0.05,
        };
        assert!(!b.is_online());
    }
}
