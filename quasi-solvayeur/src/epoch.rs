// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! One ATW epoch: compile, evolve, measure, decode.
//!
//! An epoch compiles the scheduling Hamiltonian through Afana, produces a
//! simulated measurement, and decodes it to a backend index.

use crate::bias::decode_backend;
use crate::hamiltonian::{build_scheduling_program, AtwParams};

/// The result of one ATW epoch.
#[derive(Debug, Clone)]
pub struct EpochResult {
    /// Which backend was selected.
    pub backend_index: usize,
    /// The raw measurement bitstring.
    pub measurement: Vec<bool>,
    /// The compiled QASM for this epoch's scheduling circuit.
    pub qasm: String,
    /// Number of gates in the scheduling circuit.
    pub gate_count: usize,
    /// Whether this epoch ran on classical sim or QPU.
    pub ran_on_qpu: bool,
}

/// Configuration for QPU execution.
#[derive(Debug, Clone, Default)]
pub struct QpuConfig {
    /// Path to the QPU submission script (e.g., "/home/vops/solvayeur-qpu.py").
    /// When set, the Solvayeur submits its scheduling circuit to a real QPU.
    /// When None, measurements are simulated from the bias fields.
    pub script_path: Option<String>,
    /// Number of shots per QPU submission.
    pub shots: u32,
    /// Backend name (e.g., "ibm_strasbourg").
    pub backend: String,
}

/// Execute one ATW epoch: compile -> evolve -> measure -> decode.
///
/// When `qpu_config.script_path` is set, the scheduling circuit is submitted
/// to a real QPU. Otherwise, measurements are simulated from the bias fields.
pub fn run_epoch(params: &AtwParams, epoch: usize) -> Result<EpochResult, EpochError> {
    run_epoch_with_config(params, epoch, &QpuConfig::default())
}

/// Execute one ATW epoch with QPU configuration.
pub fn run_epoch_with_config(
    params: &AtwParams,
    epoch: usize,
    qpu_config: &QpuConfig,
) -> Result<EpochResult, EpochError> {
    // 1. COMPILE: Build EhrenfestProgram from scheduling Hamiltonian
    let program = build_scheduling_program(params);

    // 2. EVOLVE: Trotterize to gate sequence
    let ast = afana::trotter::trotterize(&program, afana::trotter::TrotterOrder::First);

    // 3. EMIT: Generate QASM (for QPU submission or inspection)
    let qasm = afana::emit::emit_qasm(&ast, afana::emit::QasmVersion::V3)
        .map_err(|e| EpochError::Compile(e.to_string()))?;

    let gate_count = ast.gates.len();

    // 4. MEASURE: Either real QPU or simulated
    let (measurement, ran_on_qpu) = if let Some(script) = &qpu_config.script_path {
        match submit_to_qpu(script, &qasm, qpu_config.shots, &qpu_config.backend) {
            Ok(bits) => (bits, true),
            Err(e) => {
                tracing::warn!("QPU submission failed, falling back to simulation: {e}");
                (simulate_measurement(params, epoch), false)
            }
        }
    } else {
        (simulate_measurement(params, epoch), false)
    };

    // 5. DECODE: Bitstring -> backend index
    let backend_index = decode_backend(&measurement, params.n_backends);

    Ok(EpochResult {
        backend_index,
        measurement,
        qasm,
        gate_count,
        ran_on_qpu,
    })
}

/// Submit the scheduling QASM to a real QPU via an external script.
///
/// The script takes a QASM file and returns JSON counts (e.g., {"00": 52, "01": 23}).
/// We pick the majority bitstring and decode it to a measurement vector.
fn submit_to_qpu(
    script_path: &str,
    qasm: &str,
    shots: u32,
    backend: &str,
) -> Result<Vec<bool>, EpochError> {
    use std::process::Command;

    // Write QASM to temp file
    let qasm_path = "/tmp/solvayeur-atw.qasm";
    std::fs::write(qasm_path, qasm)
        .map_err(|e| EpochError::Execute(format!("Failed to write QASM: {e}")))?;

    // Call the QPU script
    let output = Command::new(script_path)
        .arg(qasm_path)
        .arg("--shots")
        .arg(shots.to_string())
        .arg("--backend")
        .arg(backend)
        .output()
        .map_err(|e| EpochError::Execute(format!("Failed to run QPU script: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EpochError::Execute(format!("QPU script failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let counts: std::collections::HashMap<String, u64> =
        serde_json::from_str(stdout.trim())
            .map_err(|e| EpochError::Execute(format!("Failed to parse QPU result: {e}")))?;

    // Majority vote: pick the bitstring with the most counts
    let majority = counts
        .iter()
        .max_by_key(|(_, &c)| c)
        .map(|(bs, _)| bs.clone())
        .unwrap_or_default();

    tracing::info!(
        "QPU measurement: {} (counts: {:?})",
        majority,
        counts.iter().take(4).collect::<Vec<_>>()
    );

    // Decode bitstring to bool vector (LSB first)
    let measurement: Vec<bool> = majority
        .chars()
        .rev()
        .map(|c| c == '1')
        .collect();

    Ok(measurement)
}

/// Simulate a measurement outcome based on bias fields.
///
/// Uses the bias as a probability: P(|1>_i) = sigmoid(-h_i).
/// When bias h_i is large and positive, qubit i tends to |0>.
/// When bias h_i is large and negative, qubit i tends to |1>.
/// When bias is zero, 50/50 (maximum exploration).
fn simulate_measurement(params: &AtwParams, epoch: usize) -> Vec<bool> {
    // Simulate quantum measurement using bias fields + exploration noise.
    // In production this would be actual QPU measurement of the Trotterized
    // scheduling Hamiltonian.
    //
    // The exploration term (Γ) creates a probability of flipping each qubit
    // away from the bias-preferred state, mimicking the transverse field
    // in the Hamiltonian creating superposition.
    params
        .bias
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            // sigmoid: P(|1⟩) = 1 / (1 + e^h)
            let p1 = 1.0 / (1.0 + h.exp());

            // Pseudo-random threshold from exploration + epoch + qubit index.
            // Uses a simple hash-like function to create deterministic but
            // varied exploration patterns across epochs.
            let hash = ((epoch * 7919 + i * 104729 + 31) % 1000) as f64 / 1000.0;
            // Exploration mixes: when Γ is high, threshold varies widely (more randomness).
            // When Γ is low, threshold is near 0.5 (bias dominates).
            let exploration_range = params.exploration.min(1.0) * 0.5;
            let threshold = 0.5 + exploration_range * (hash - 0.5);
            p1 > threshold
        })
        .collect()
}

/// Errors during an ATW epoch.
#[derive(Debug, thiserror::Error)]
pub enum EpochError {
    /// Compilation through Afana failed.
    #[error("compilation failed: {0}")]
    Compile(String),
    /// Execution on backend failed.
    #[error("execution failed: {0}")]
    Execute(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hamiltonian::AtwParams;
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

    #[test]
    fn run_epoch_compiles_and_returns_valid_result() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params, 0).unwrap();

        assert!(result.backend_index < 4);
        assert_eq!(result.measurement.len(), 2);
        assert!(result.gate_count > 0);
        assert!(!result.ran_on_qpu);
    }

    #[test]
    fn run_epoch_zero_bias_produces_valid_index() {
        let backends = make_backends(8);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params, 0).unwrap();

        assert!(result.backend_index < 8);
    }

    #[test]
    fn run_epoch_qasm_output_is_valid_qasm3() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params, 0).unwrap();

        assert!(result.qasm.contains("OPENQASM 3.0;"));
        assert!(result.qasm.contains("include \"stdgates.inc\";"));
        assert!(result.qasm.contains("qubit["));
    }
}
