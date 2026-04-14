// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Backend capability model.
//!
//! Re-exports HAL Contract v2 types as the canonical source of truth,
//! plus QUASI-specific extensions for scheduling (cost, status).
//!
//! **All backend types in QUASI come from the HAL Contract spec.**
//! If you need a new field, add it to the HAL Contract spec first,
//! then use it here. No workarounds, no version drift.

// ── Re-exports from HAL Contract v2 ─────────────────────────────────────────

pub use hal_contract::{
    Capabilities, GateSet, HalError, HalResult, NoiseProfile, Topology, TopologyKind,
};

// ── QUASI-specific extensions ───────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// Unique backend identifier.
pub type BackendId = String;

/// Current availability status (QUASI scheduling view).
///
/// This is a simplified projection of `hal_contract::BackendAvailability`
/// for use in the scheduler's filter-score-bind pipeline. The full HAL
/// availability (with estimated_wait, status_message) is obtained via
/// the Backend trait's `availability()` method at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendStatus {
    /// Backend is online with a queue depth.
    Online { queue_depth: u32 },
    /// Backend is under maintenance.
    Maintenance,
    /// Backend is offline.
    Offline,
}

/// Hardware type — used by the scheduler to distinguish QPU from simulator.
///
/// Note: HAL Contract uses `Capabilities.is_simulator` (bool).
/// This enum provides a more readable API for scheduling plugins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendType {
    /// Real quantum processing unit.
    Qpu,
    /// Classical simulator.
    Simulator,
}

/// A registered backend with HAL Contract capabilities plus scheduling metadata.
///
/// The `capabilities` field holds the canonical HAL Contract v2 types.
/// The remaining fields are QUASI scheduling extensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backend {
    /// HAL Contract capabilities (gate set, topology, noise profile, etc.)
    pub capabilities: Capabilities,
    /// QUASI scheduling: hardware type.
    pub backend_type: BackendType,
    /// QUASI scheduling: current availability status.
    pub status: BackendStatus,
    /// QUASI scheduling: cost per measurement shot (for cost-aware routing).
    pub cost_per_shot: f64,
}

impl Backend {
    /// Backend identifier (from HAL Contract capabilities.name).
    pub fn id(&self) -> &str {
        &self.capabilities.name
    }

    /// Number of qubits (from HAL Contract capabilities).
    pub fn qubit_count(&self) -> u32 {
        self.capabilities.num_qubits
    }

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

    /// Whether the gate set supports a specific gate.
    pub fn supports_gate(&self, gate: &str) -> bool {
        let gs = &self.capabilities.gate_set;
        gs.single_qubit.iter().any(|g| g == gate)
            || gs.two_qubit.iter().any(|g| g == gate)
            || gs.three_qubit.iter().any(|g| g == gate)
            || gs.native.iter().any(|g| g == gate)
    }

    /// Whether the gate set supports all given gates.
    pub fn supports_all_gates(&self, gates: &[String]) -> bool {
        gates.iter().all(|g| self.supports_gate(g))
    }

    /// Noise profile (from HAL Contract capabilities).
    pub fn noise_profile(&self) -> Option<&NoiseProfile> {
        self.capabilities.noise_profile.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_backend(name: &str, backend_type: BackendType, status: BackendStatus) -> Backend {
        Backend {
            capabilities: Capabilities {
                name: name.into(),
                num_qubits: 10,
                gate_set: GateSet {
                    single_qubit: vec!["h".into(), "rz".into()],
                    two_qubit: vec!["cx".into()],
                    three_qubit: vec![],
                    native: vec!["rz".into(), "sx".into(), "cx".into()],
                },
                topology: Topology {
                    kind: TopologyKind::FullyConnected,
                    edges: vec![],
                },
                max_shots: 10_000,
                is_simulator: matches!(backend_type, BackendType::Simulator),
                features: vec![],
                noise_profile: Some(NoiseProfile {
                    t1: Some(100.0),
                    t2: Some(50.0),
                    single_qubit_fidelity: Some(0.999),
                    two_qubit_fidelity: Some(0.99),
                    readout_fidelity: Some(0.98),
                    gate_time: Some(0.05),
                }),
            },
            backend_type,
            status,
            cost_per_shot: 0.01,
        }
    }

    #[test]
    fn backend_id_from_capabilities() {
        let b = make_backend("test_qpu", BackendType::Qpu, BackendStatus::Online { queue_depth: 3 });
        assert_eq!(b.id(), "test_qpu");
        assert_eq!(b.qubit_count(), 10);
    }

    #[test]
    fn gate_set_supports() {
        let b = make_backend("test", BackendType::Qpu, BackendStatus::Online { queue_depth: 0 });
        assert!(b.supports_gate("h"));
        assert!(b.supports_gate("cx"));
        assert!(b.supports_gate("rz"));
        assert!(!b.supports_gate("t"));
    }

    #[test]
    fn gate_set_supports_all() {
        let b = make_backend("test", BackendType::Qpu, BackendStatus::Online { queue_depth: 0 });
        assert!(b.supports_all_gates(&["h".into(), "cx".into()]));
        assert!(!b.supports_all_gates(&["h".into(), "t".into()]));
    }

    #[test]
    fn backend_is_online() {
        let b = make_backend("test", BackendType::Qpu, BackendStatus::Online { queue_depth: 5 });
        assert!(b.is_online());
        assert_eq!(b.queue_depth(), 5);
    }

    #[test]
    fn backend_not_online_when_maintenance() {
        let b = make_backend("test", BackendType::Simulator, BackendStatus::Maintenance);
        assert!(!b.is_online());
        assert_eq!(b.queue_depth(), u32::MAX);
    }

    #[test]
    fn backend_not_online_when_offline() {
        let b = make_backend("test", BackendType::Qpu, BackendStatus::Offline);
        assert!(!b.is_online());
    }

    #[test]
    fn noise_profile_accessible() {
        let b = make_backend("test", BackendType::Qpu, BackendStatus::Online { queue_depth: 0 });
        let noise = b.noise_profile().unwrap();
        assert_eq!(noise.t1, Some(100.0));
        assert_eq!(noise.single_qubit_fidelity, Some(0.999));
    }
}
