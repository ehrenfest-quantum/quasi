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

/// Execute one ATW epoch: compile -> evolve -> measure -> decode.
///
/// This compiles the scheduling Hamiltonian through Afana and returns
/// a simulated measurement outcome. In production, the measurement
/// would come from actual QPU execution.
pub fn run_epoch(params: &AtwParams) -> Result<EpochResult, EpochError> {
    // 1. COMPILE: Build EhrenfestProgram from scheduling Hamiltonian
    let program = build_scheduling_program(params);

    // 2. EVOLVE: Trotterize to gate sequence
    let ast = afana::trotter::trotterize(&program, afana::trotter::TrotterOrder::First);

    // 3. EMIT: Generate QASM (for QPU submission or inspection)
    let qasm = afana::emit::emit_qasm(&ast, afana::emit::QasmVersion::V3)
        .map_err(|e| EpochError::Compile(e.to_string()))?;

    let gate_count = ast.gates.len();

    // 4. MEASURE: Simulate measurement from the compiled circuit.
    //    In a full implementation, this would:
    //    a) Check routing confidence (Huoma profiler)
    //    b) Run on Huoma (classical) or QPU (quantum) accordingly
    //    c) Return actual measurement outcome
    //
    //    For now, we derive a deterministic "measurement" from the bias fields.
    //    This lets us test the full ATW loop without QPU access.
    let measurement = simulate_measurement(params);

    // 5. DECODE: Bitstring -> backend index
    let backend_index = decode_backend(&measurement, params.n_backends);

    Ok(EpochResult {
        backend_index,
        measurement,
        qasm,
        gate_count,
        ran_on_qpu: false, // simulated for now
    })
}

/// Simulate a measurement outcome based on bias fields.
///
/// Uses the bias as a probability: P(|1>_i) = sigmoid(-h_i).
/// When bias h_i is large and positive, qubit i tends to |0>.
/// When bias h_i is large and negative, qubit i tends to |1>.
/// When bias is zero, 50/50 (maximum exploration).
fn simulate_measurement(params: &AtwParams) -> Vec<bool> {
    // Deterministic pseudo-random based on bias values.
    // In production this would be actual quantum measurement.
    params
        .bias
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            // sigmoid: P(|1>) = 1 / (1 + e^h)
            // When h > 0, P(|1>) < 0.5 (prefer |0>)
            // When h < 0, P(|1>) > 0.5 (prefer |1>)
            // When h = 0, P(|1>) = 0.5 (unbiased)
            let p1 = 1.0 / (1.0 + h.exp());
            // Use a simple deterministic threshold based on qubit index
            // (Real implementation uses actual quantum measurement)
            let threshold = 0.5 + 0.1 * (i as f64 / params.n_qubits.max(1) as f64);
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
                id: format!("backend_{i}"),
                name: format!("Backend {i}"),
                backend_type: BackendType::Qpu,
                qubit_count: 20,
                gate_set: GateSet {
                    native_gates: vec!["h".into(), "cx".into(), "rz".into()],
                },
                topology: Topology::AllToAll,
                noise: NoiseProfile {
                    t1_us: 200.0,
                    t2_us: 100.0,
                    single_qubit_error: 0.001,
                    two_qubit_error: 0.01,
                    readout_error: 0.02,
                    calibration_version: "v1".into(),
                },
                status: BackendStatus::Online { queue_depth: 0 },
                cost_per_shot: 0.01,
            })
            .collect()
    }

    #[test]
    fn run_epoch_compiles_and_returns_valid_result() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params).unwrap();

        assert!(result.backend_index < 4);
        assert_eq!(result.measurement.len(), 2);
        assert!(result.gate_count > 0);
        assert!(!result.ran_on_qpu);
    }

    #[test]
    fn run_epoch_zero_bias_produces_valid_index() {
        let backends = make_backends(8);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params).unwrap();

        assert!(result.backend_index < 8);
    }

    #[test]
    fn run_epoch_qasm_output_is_valid_qasm3() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let result = run_epoch(&params).unwrap();

        assert!(result.qasm.contains("OPENQASM 3.0;"));
        assert!(result.qasm.contains("include \"stdgates.inc\";"));
        assert!(result.qasm.contains("qubit["));
    }
}
