// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Scheduling Hamiltonian constructor for the ATW algorithm.
//!
//! Builds an `EhrenfestProgram` (from `afana::cbor`) encoding the scheduling
//! problem as a transverse-field Ising model:
//!
//! ```text
//! H(k) = Σ_ij J_ij Z_i Z_j  +  Σ_i h_i(k) Z_i  +  Γ Σ_i X_i
//!         (contention)          (learned bias)      (exploration)
//! ```

use afana::cbor::*;
use quasi_scheduler::backend::Backend;

/// Parameters for the ATW scheduling Hamiltonian.
#[derive(Debug, Clone)]
pub struct AtwParams {
    /// Number of qubits (ceil(log2(num_backends))).
    pub n_qubits: usize,
    /// Number of backends.
    pub n_backends: usize,
    /// Contention coupling strengths J_ij.
    pub contention: Vec<f64>,
    /// Current bias fields h_i (learned, updated each epoch).
    pub bias: Vec<f64>,
    /// Exploration strength (transverse field Gamma).
    pub exploration: f64,
    /// Evolution time for Trotter.
    pub evolution_time: f64,
    /// Trotter steps.
    pub trotter_steps: u32,
}

impl AtwParams {
    /// Create initial ATW parameters for a set of backends.
    ///
    /// Contention J_ij starts at a small coupling value.
    /// Bias h_i starts at zero (no preference).
    /// Exploration Gamma starts at 1.0 (maximum exploration).
    pub fn from_backends(backends: &[Backend]) -> Self {
        let n = backends.len();
        let n_qubits = if n <= 1 {
            1
        } else {
            (n as f64).log2().ceil() as usize
        };

        // Contention: one coupling per qubit pair
        let n_pairs = n_qubits * (n_qubits.saturating_sub(1)) / 2;
        let contention = vec![0.1; n_pairs];

        Self {
            n_qubits,
            n_backends: n,
            contention,
            bias: vec![0.0; n_qubits],
            exploration: 1.0,
            evolution_time: 1.0,
            trotter_steps: 5,
        }
    }
}

/// Build an `EhrenfestProgram` encoding the ATW scheduling Hamiltonian.
///
/// The Hamiltonian has three parts:
///   H = Sigma_ij J_ij Z_i Z_j + Sigma_i h_i Z_i + Gamma Sigma_i X_i
///
/// This is a standard transverse-field Ising model where:
/// - ZZ terms encode backend contention
/// - Z terms encode learned bias (from previous ATW rounds)
/// - X terms encode exploration (transverse field)
pub fn build_scheduling_program(params: &AtwParams) -> EhrenfestProgram {
    let mut terms = Vec::new();

    // ZZ contention terms: Sigma_ij J_ij Z_i Z_j
    let mut pair_idx = 0;
    for i in 0..params.n_qubits {
        for j in (i + 1)..params.n_qubits {
            if pair_idx < params.contention.len() {
                let coeff = params.contention[pair_idx];
                if coeff.abs() > 1e-12 {
                    terms.push(PauliTerm {
                        coefficient: coeff,
                        paulis: vec![
                            PauliOpEntry {
                                qubit: i,
                                axis: PauliOp::Z,
                            },
                            PauliOpEntry {
                                qubit: j,
                                axis: PauliOp::Z,
                            },
                        ],
                    });
                }
                pair_idx += 1;
            }
        }
    }

    // Z bias terms: Sigma_i h_i Z_i
    for (i, &h) in params.bias.iter().enumerate() {
        if h.abs() > 1e-12 {
            terms.push(PauliTerm {
                coefficient: h,
                paulis: vec![PauliOpEntry {
                    qubit: i,
                    axis: PauliOp::Z,
                }],
            });
        }
    }

    // X exploration terms: Gamma Sigma_i X_i
    if params.exploration.abs() > 1e-12 {
        for i in 0..params.n_qubits {
            terms.push(PauliTerm {
                coefficient: params.exploration,
                paulis: vec![PauliOpEntry {
                    qubit: i,
                    axis: PauliOp::X,
                }],
            });
        }
    }

    // If no terms, add a trivial identity-like term to avoid empty Hamiltonian
    if terms.is_empty() {
        terms.push(PauliTerm {
            coefficient: 0.0,
            paulis: vec![PauliOpEntry {
                qubit: 0,
                axis: PauliOp::I,
            }],
        });
    }

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: params.n_qubits,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: params.evolution_time,
            steps: params.trotter_steps,
            dt_us: params.evolution_time / params.trotter_steps as f64,
        },
        observables: (0..params.n_qubits)
            .map(|q| Observable::SZ { qubit: q })
            .collect(),
        noise: NoiseConstraint {
            t1_us: 1e6, // generous — scheduling circuit is tiny
            t2_us: 1e6,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                status: BackendStatus::Online { queue_depth: i as u32 },
                cost_per_shot: 0.01,
            })
            .collect()
    }

    #[test]
    fn from_backends_4_gives_2_qubits() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        assert_eq!(params.n_qubits, 2);
        assert_eq!(params.n_backends, 4);
        assert_eq!(params.bias, vec![0.0; 2]);
        // 2 qubits → 1 pair
        assert_eq!(params.contention.len(), 1);
    }

    #[test]
    fn from_backends_8_gives_3_qubits() {
        let backends = make_backends(8);
        let params = AtwParams::from_backends(&backends);
        assert_eq!(params.n_qubits, 3);
        assert_eq!(params.n_backends, 8);
        // 3 qubits → 3 pairs
        assert_eq!(params.contention.len(), 3);
    }

    #[test]
    fn from_backends_1_gives_1_qubit() {
        let backends = make_backends(1);
        let params = AtwParams::from_backends(&backends);
        assert_eq!(params.n_qubits, 1);
    }

    #[test]
    fn build_scheduling_program_produces_valid_program() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let program = build_scheduling_program(&params);

        assert_eq!(program.version, 1);
        assert_eq!(program.system.n_qubits, 2);
        assert!(!program.hamiltonian.terms.is_empty());
        assert_eq!(program.observables.len(), 2);
    }

    #[test]
    fn build_scheduling_program_has_correct_term_count() {
        let backends = make_backends(4);
        let params = AtwParams::from_backends(&backends);
        let program = build_scheduling_program(&params);

        // 2 qubits: 1 ZZ term + 0 Z terms (bias=0) + 2 X terms = 3
        assert_eq!(program.hamiltonian.terms.len(), 3);
    }

    #[test]
    fn build_scheduling_program_with_bias_has_z_terms() {
        let backends = make_backends(4);
        let mut params = AtwParams::from_backends(&backends);
        params.bias = vec![0.5, -0.3];
        let program = build_scheduling_program(&params);

        // 1 ZZ + 2 Z (bias) + 2 X = 5
        assert_eq!(program.hamiltonian.terms.len(), 5);
    }

    #[test]
    fn build_scheduling_program_no_exploration_omits_x_terms() {
        let backends = make_backends(4);
        let mut params = AtwParams::from_backends(&backends);
        params.exploration = 0.0;
        let program = build_scheduling_program(&params);

        // 1 ZZ + 0 Z (bias=0) + 0 X = 1
        assert_eq!(program.hamiltonian.terms.len(), 1);
    }

    #[test]
    fn build_scheduling_program_empty_produces_identity() {
        let mut params = AtwParams::from_backends(&make_backends(1));
        params.exploration = 0.0;
        params.contention.clear();
        let program = build_scheduling_program(&params);

        // Should have at least one term (identity fallback)
        assert!(!program.hamiltonian.terms.is_empty());
    }
}
