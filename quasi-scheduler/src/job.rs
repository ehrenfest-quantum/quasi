// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Job definition and scheduling decisions.
//!
//! Represents the resource requirements of a quantum job and the possible
//! outcomes of a scheduling decision.

use serde::{Deserialize, Serialize};

use crate::backend::BackendId;

/// Noise constraints the backend must satisfy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseBudget {
    /// Minimum T1 relaxation time in microseconds.
    pub min_t1_us: f64,
    /// Minimum T2 dephasing time in microseconds.
    pub min_t2_us: f64,
    /// Maximum tolerable gate error rate.
    pub max_gate_error: Option<f64>,
    /// Minimum required fidelity.
    pub min_fidelity: Option<f64>,
}

/// Resource requirements for a quantum job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequirements {
    /// Minimum number of qubits needed.
    pub min_qubits: u32,
    /// Gates the backend must support natively.
    pub required_gates: Vec<String>,
    /// Circuit depth (layer count).
    pub circuit_depth: u32,
    /// Number of shots requested.
    pub shot_count: u32,
    /// Noise constraints from the Ehrenfest program.
    pub noise_budget: Option<NoiseBudget>,
    /// Whether the job prefers a real QPU over a simulator.
    pub prefers_qpu: bool,
    /// Maximum total cost for all shots.
    pub max_cost: Option<f64>,
}

/// A quantum job to be scheduled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumJob {
    /// Unique job identifier.
    pub id: String,
    /// Hash of the compiled circuit (for cache lookup).
    pub circuit_hash: [u8; 32],
    /// Resource requirements.
    pub requirements: JobRequirements,
    /// Priority (higher = more urgent).
    pub priority: u32,
    /// Unix timestamp when the job was submitted.
    pub submitted_at: u64,
}

/// Result of scheduling a job.
#[derive(Debug, Clone)]
pub enum ScheduleDecision {
    /// Run on this backend.
    Assign { backend_id: BackendId, score: f64 },
    /// Result is cached, skip execution.
    CacheHit { cache_key: String },
    /// No suitable backend available right now.
    Pending { reason: String },
    /// Cannot be scheduled (hard constraint violation).
    Rejected { reason: String },
}
