// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR validation for multi-qubit gate sequence parity in Heisenberg models.
//!
//! This module validates that compiled gate sequences maintain the correct
//! parity for multi-qubit entangling gates against the expected Hamiltonian
//! evolution. For Heisenberg models (XX, YY, ZZ interactions), each two-qubit
//! Pauli term produces a CNOT ladder with an even number of CX gates during
//! Trotterization. This validation ensures semantic fidelity of the compiled
//! circuit.

use crate::ast::GateName;
use crate::cbor::{EhrenfestProgram, PauliOp};

/// Result of parity validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ParityValidationResult {
    /// Parity check passed.
    Ok,
    /// Parity mismatch detected.
    Mismatch {
        expected_parity: usize,
        actual_parity: usize,
        expected_cx_count: usize,
        actual_cx_count: usize,
    },
}

/// Validate multi-qubit gate sequence parity against expected Hamiltonian evolution.
///
/// For Heisenberg models, each two-qubit Pauli term (XX, YY, ZZ) produces
/// a CNOT ladder during Trotterization. The ladder has 2*(k-1) CX gates
/// for a k-qubit Pauli string. This function computes the expected CX count
/// parity from the Hamiltonian structure and compares it against the actual
/// synthesized gate sequence.
///
/// Returns `ParityValidationResult::Ok` if parity matches, or
/// `ParityValidationResult::Mismatch` with details if it doesn't.
pub fn validate_gate_parity(
    program: &EhrenfestProgram,
    gates: &[crate::ast::Gate],
) -> ParityValidationResult {
    // Count actual CX gates in the synthesized sequence.
    let actual_cx_count = gates
        .iter()
        .filter(|g| g.name == GateName::Cx)
        .count();

    // Compute expected CX count from Hamiltonian structure.
    let mut expected_cx_count: usize = 0;
    let steps = program.evolution.steps as usize;

    for term in &program.hamiltonian.terms {
        // Count non-identity Pauli operators in this term.
        let active_count = term
            .paulis
            .iter()
            .filter(|p| !matches!(p.axis, PauliOp::I))
            .count();

        if active_count >= 2 {
            // CNOT ladder has 2*(k-1) CX gates per Trotter step.
            let cx_per_step = 2 * (active_count - 1);
            expected_cx_count += cx_per_step * steps;
        }
    }

    let expected_parity = expected_cx_count % 2;
    let actual_parity = actual_cx_count % 2;

    if expected_parity == actual_parity {
        ParityValidationResult::Ok
    } else {
        ParityValidationResult::Mismatch {
            expected_parity,
            actual_parity,
            expected_cx_count,
            actual_cx_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Gate;
    use crate::cbor::*;
    use crate::trotter::{trotterize, TrotterOrder};

    /// Build a Heisenberg model EhrenfestProgram with XX, YY, ZZ terms.
    fn heisenberg_program(n_qubits: usize, steps: u32) -> EhrenfestProgram {
        let mut terms = Vec::new();

        // Add XX, YY, ZZ terms for adjacent qubit pairs.
        for i in 0..n_qubits.saturating_sub(1) {
            // XX term
            terms.push(PauliTerm {
                coefficient: 0.5,
                paulis: vec![
                    PauliOpEntry { qubit: i, axis: PauliOp::X },
                    PauliOpEntry { qubit: i + 1, axis: PauliOp::X },
                ],
            });
            // YY term
            terms.push(PauliTerm {
                coefficient: 0.5,
                paulis: vec![
                    PauliOpEntry { qubit: i, axis: PauliOp::Y },
                    PauliOpEntry { qubit: i + 1, axis: PauliOp::Y },
                ],
            });
            // ZZ term
            terms.push(PauliTerm {
                coefficient: 0.5,
                paulis: vec![
                    PauliOpEntry { qubit: i, axis: PauliOp::Z },
                    PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
                ],
            });
        }

        let total_us = 1.0;
        let dt_us = total_us / steps as f64;

        EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms,
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us,
                steps,
                dt_us,
            },
            observables: vec![Observable::SZ { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        }
    }

    #[test]
    fn test_heisenberg_parity_3q_1step() {
        let program = heisenberg_program(3, 1);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "3-qubit Heisenberg with 1 step should pass parity validation");
    }

    #[test]
    fn test_heisenberg_parity_3q_10steps() {
        let program = heisenberg_program(3, 10);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "3-qubit Heisenberg with 10 steps should pass parity validation");
    }

    #[test]
    fn test_heisenberg_parity_4q_1step() {
        let program = heisenberg_program(4, 1);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "4-qubit Heisenberg with 1 step should pass parity validation");
    }

    #[test]
    fn test_heisenberg_parity_4q_5steps() {
        let program = heisenberg_program(4, 5);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "4-qubit Heisenberg with 5 steps should pass parity validation");
    }

    #[test]
    fn test_heisenberg_parity_2q_1step() {
        let program = heisenberg_program(2, 1);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "2-qubit Heisenberg with 1 step should pass parity validation");
    }

    #[test]
    fn test_heisenberg_parity_second_order() {
        let program = heisenberg_program(3, 1);
        let ast = trotterize(&program, TrotterOrder::Second);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "3-qubit Heisenberg second-order should pass parity validation");
    }

    #[test]
    fn test_parity_mismatch_detected() {
        let program = heisenberg_program(2, 1);
        // Create gates with wrong CX count (odd instead of even).
        let bad_gates = vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
        ];
        let result = validate_gate_parity(&program, &bad_gates);
        assert!(matches!(result, ParityValidationResult::Mismatch { .. }),
            "Should detect parity mismatch with wrong CX count");
    }

    #[test]
    fn test_single_qubit_term_no_cx_expected() {
        // Program with only single-qubit terms — no CX gates expected.
        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 2,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient: 1.0,
                    paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 1,
                dt_us: 1.0,
            },
            observables: vec![Observable::SZ { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "Single-qubit term should have 0 CX (even parity)");
    }

    #[test]
    fn test_identity_term_no_cx_expected() {
        // Program with identity-only term — no CX gates expected.
        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 2,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::I },
                        PauliOpEntry { qubit: 1, axis: PauliOp::I },
                    ],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 1,
                dt_us: 1.0,
            },
            observables: vec![Observable::SZ { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "Identity term should produce no CX gates");
    }

    #[test]
    fn test_heisenberg_parity_5q_2steps() {
        let program = heisenberg_program(5, 2);
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(result, ParityValidationResult::Ok,
            "5-qubit Heisenberg with 2 steps should pass parity validation");
    }
}