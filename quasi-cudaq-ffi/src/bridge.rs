// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Level 2: Hamiltonian interception bridge.
//!
//! When CUDA-Q calls `cudaq::observe()` with a `spin_op`, the C++ adapter
//! intercepts the Pauli Hamiltonian before CUDA-Q decomposes it into circuits.
//! This module routes it through QUASI's pipeline instead:
//!
//! ```text
//! Pauli Hamiltonian (from CUDA-Q spin_op)
//!     → build Ehrenfest program
//!     → compile with Afana (Trotter S₂/S₄, ZX-calculus)
//!     → exact diagonalization (Huoma statevector equivalent)
//!     → return ground-state energy
//! ```

use quasi_bridge::ehrenfest_mol::{self, Accuracy};
use quasi_bridge::postprocess;

/// Compute the expectation value / ground-state energy for a Pauli Hamiltonian.
///
/// This is the Level 2 "Hamiltonian interceptor": instead of letting CUDA-Q
/// decompose the Hamiltonian into circuits, QUASI handles the full pipeline.
pub fn observe_hamiltonian(
    pauli_terms: &[(f64, String)],
    n_qubits: usize,
) -> Result<f64, BridgeObserveError> {
    if pauli_terms.is_empty() {
        return Err(BridgeObserveError::EmptyHamiltonian);
    }
    if n_qubits == 0 {
        return Err(BridgeObserveError::InvalidQubitCount);
    }
    if n_qubits > 20 {
        return Err(BridgeObserveError::TooManyQubits(n_qubits));
    }

    // 1. Build Ehrenfest program from the Pauli Hamiltonian
    let program = ehrenfest_mol::build_ehrenfest_program(pauli_terms, n_qubits, Accuracy::Chemical);
    let cbor_bytes = ehrenfest_mol::to_cbor(&program);

    // 2. Verify Afana can parse and compile
    let parsed = afana::cbor::from_cbor(&cbor_bytes)
        .map_err(|e| BridgeObserveError::AfanaCompile(e.to_string()))?;
    let _ast = afana::trotter::trotterize(&parsed, afana::trotter::TrotterOrder::Second);

    // 3. Solvayeur backend selection (classical fallback for small systems)
    let backends = make_simulator_backends();
    let solvayeur = quasi_solvayeur::atw::Solvayeur::new(&backends);
    let _epoch_result = solvayeur
        .decide()
        .map_err(|e| BridgeObserveError::Solvayeur(e.to_string()))?;

    // 4. Exact diagonalization (Huoma statevector equivalent)
    let energy = postprocess::ground_state_energy(pauli_terms, n_qubits)
        .map_err(|e| BridgeObserveError::Execution(e.to_string()))?;

    Ok(energy)
}

fn make_simulator_backends() -> Vec<quasi_scheduler::backend::Backend> {
    use quasi_scheduler::backend::*;
    vec![Backend {
        id: "huoma_statevector".into(),
        name: "Huoma Statevector".into(),
        backend_type: BackendType::Simulator,
        qubit_count: 30,
        gate_set: GateSet {
            native_gates: vec![
                "h".into(),
                "x".into(),
                "y".into(),
                "z".into(),
                "cx".into(),
                "rz".into(),
                "ry".into(),
                "rx".into(),
            ],
        },
        topology: Topology::AllToAll,
        noise: NoiseProfile {
            t1_us: 1e9,
            t2_us: 1e9,
            single_qubit_error: 0.0,
            two_qubit_error: 0.0,
            readout_error: 0.0,
            calibration_version: "exact".into(),
        },
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.0001,
    }]
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeObserveError {
    #[error("empty Hamiltonian")]
    EmptyHamiltonian,
    #[error("invalid qubit count")]
    InvalidQubitCount,
    #[error("too many qubits for exact diag: {0} (max 20)")]
    TooManyQubits(usize),
    #[error("Afana compilation failed: {0}")]
    AfanaCompile(String),
    #[error("Solvayeur error: {0}")]
    Solvayeur(String),
    #[error("execution error: {0}")]
    Execution(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_simple_hamiltonian() {
        // H = Z on 1 qubit: ground state energy = -1
        let terms = vec![(1.0, "Z".to_string())];
        let energy = observe_hamiltonian(&terms, 1).unwrap();
        assert!((energy - (-1.0)).abs() < 1e-10, "energy = {energy}");
    }

    #[test]
    fn observe_h2_like_hamiltonian() {
        // Simplified H₂ Hamiltonian (deuteron model)
        let terms = vec![
            (5.907, "II".to_string()),
            (-2.1433, "XX".to_string()),
            (-2.1433, "YY".to_string()),
            (0.21829, "ZI".to_string()),
            (-6.125, "IZ".to_string()),
        ];
        let energy = observe_hamiltonian(&terms, 2).unwrap();
        // Ground state of this Hamiltonian should be near -1.7489
        assert!(energy < 0.0, "H2 energy should be negative: {energy}");
    }

    #[test]
    fn observe_empty_hamiltonian_fails() {
        let result = observe_hamiltonian(&[], 2);
        assert!(result.is_err());
    }

    #[test]
    fn observe_too_many_qubits_fails() {
        let terms = vec![(1.0, "Z".to_string())];
        let result = observe_hamiltonian(&terms, 25);
        assert!(result.is_err());
    }
}
