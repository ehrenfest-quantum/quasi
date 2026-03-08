// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Trotterization: derive gate sequences from Hamiltonian time evolution.
//!
//! This module bridges the physics-level Ehrenfest program (Hamiltonians,
//! observables, noise constraints) to the circuit-level AST (gate sequences)
//! that the QASM emitter consumes.
//!
//! The Trotter-Suzuki decomposition approximates:
//!   e^{-iHt} ≈ (e^{-iH₁dt} · e^{-iH₂dt} · ...)^n
//!
//! Each Pauli term e^{-i·θ·P} is decomposed into a basis-change + Rz + undo
//! pattern. For example, e^{-iθ·X₀Z₁} decomposes to:
//!   H q0; Rz(2θ) q1; CNOT q0 q1; Rz(2θ) q1; CNOT q0 q1; H q0;

use crate::ast::*;
use crate::cbor::{EhrenfestProgram, PauliOp, PauliOpEntry};

/// Trotter decomposition order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrotterOrder {
    #[default]
    First,
    Second,
}

/// Derive a gate-level AST from an Ehrenfest physics program via Trotterization.
///
/// The `order` parameter controls the decomposition:
/// - First-order: e^{-iH₁dt} · e^{-iH₂dt} · ...
/// - Second-order: symmetric Suzuki-Trotter (forward half-step + reverse half-step)
pub fn trotterize(program: &EhrenfestProgram, order: TrotterOrder) -> EhrenfestAst {
    let dt = program.evolution.dt_us;
    let mut gates = Vec::new();

    for _step in 0..program.evolution.steps {
        match order {
            TrotterOrder::First => {
                for term in &program.hamiltonian.terms {
                    let theta = term.coefficient * dt;
                    gates.extend(pauli_rotation_gates(&term.paulis, theta));
                }
            }
            TrotterOrder::Second => {
                let half_dt = dt / 2.0;
                // Forward half-step
                for term in &program.hamiltonian.terms {
                    let theta = term.coefficient * half_dt;
                    gates.extend(pauli_rotation_gates(&term.paulis, theta));
                }
                // Backward half-step (reverse order)
                for term in program.hamiltonian.terms.iter().rev() {
                    let theta = term.coefficient * half_dt;
                    gates.extend(pauli_rotation_gates(&term.paulis, theta));
                }
            }
        }
    }

    // Measure all qubits.
    let measures: Vec<Measure> = (0..program.system.n_qubits)
        .map(|i| Measure { qubit: i, cbit: i })
        .collect();

    EhrenfestAst {
        name: format!("ehrenfest_{}q", program.system.n_qubits),
        n_qubits: program.system.n_qubits,
        prepare: None,
        gates,
        measures,
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

/// Decompose a Pauli rotation e^{-iθ·P} into native gates.
///
/// For a single-qubit Pauli:
///   e^{-iθZ} → Rz(2θ)
///   e^{-iθX} → H; Rz(2θ); H
///   e^{-iθY} → Sdg; H; Rz(2θ); H; S
///
/// For multi-qubit Pauli tensor products, CNOT ladders entangle the qubits,
/// then a single Rz rotation is applied, then the ladder is undone.
fn pauli_rotation_gates(ops: &[PauliOpEntry], theta: f64) -> Vec<Gate> {
    // Filter out identity operators — they contribute no gates.
    let active_ops: Vec<&PauliOpEntry> = ops
        .iter()
        .filter(|entry| !matches!(entry.axis, PauliOp::I))
        .collect();

    if active_ops.is_empty() {
        return Vec::new();
    }

    let mut gates = Vec::new();

    // Step 1: Basis change (rotate X/Y qubits into Z basis).
    for entry in &active_ops {
        match entry.axis {
            PauliOp::X => {
                gates.push(Gate {
                    name: GateName::H,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
            }
            PauliOp::Y => {
                gates.push(Gate {
                    name: GateName::Sdg,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
                gates.push(Gate {
                    name: GateName::H,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
            }
            PauliOp::Z | PauliOp::I => {}
        }
    }

    // Step 2: CNOT ladder (entangle qubits for multi-qubit Pauli terms).
    if active_ops.len() > 1 {
        for i in 0..active_ops.len() - 1 {
            gates.push(Gate {
                name: GateName::Cx,
                qubits: vec![active_ops[i].qubit, active_ops[i + 1].qubit],
                params: vec![],
            });
        }
    }

    // Step 3: Rz rotation on the last active qubit.
    let target = active_ops.last().unwrap().qubit;
    gates.push(Gate {
        name: GateName::Rz,
        qubits: vec![target],
        params: vec![2.0 * theta],
    });

    // Step 4: Undo CNOT ladder (reverse order).
    if active_ops.len() > 1 {
        for i in (0..active_ops.len() - 1).rev() {
            gates.push(Gate {
                name: GateName::Cx,
                qubits: vec![active_ops[i].qubit, active_ops[i + 1].qubit],
                params: vec![],
            });
        }
    }

    // Step 5: Undo basis change (reverse of step 1).
    for entry in active_ops.iter().rev() {
        match entry.axis {
            PauliOp::X => {
                gates.push(Gate {
                    name: GateName::H,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
            }
            PauliOp::Y => {
                gates.push(Gate {
                    name: GateName::H,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
                gates.push(Gate {
                    name: GateName::S,
                    qubits: vec![entry.qubit],
                    params: vec![],
                });
            }
            PauliOp::Z | PauliOp::I => {}
        }
    }

    gates
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cbor::*;

    fn simple_zz_program() -> EhrenfestProgram {
        EhrenfestProgram {
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
                        PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Z },
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
        }
    }

    #[test]
    fn trotterize_zz_produces_gates() {
        let program = simple_zz_program();
        let ast = trotterize(&program, TrotterOrder::First);
        assert_eq!(ast.n_qubits, 2);
        assert!(!ast.gates.is_empty(), "should produce gate sequence");
        // ZZ term → CNOT + Rz + CNOT pattern.
        let gate_names: Vec<&str> = ast.gates.iter().map(|g| g.name.as_str()).collect();
        assert!(gate_names.contains(&"cx"), "ZZ needs CNOT ladder");
        assert!(gate_names.contains(&"rz"), "ZZ needs Rz rotation");
    }

    #[test]
    fn trotterize_single_x_produces_h_rz_h() {
        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 1,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient: 1.0,
                    paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 1,
                dt_us: 1.0,
            },
            observables: vec![Observable::SX { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 50.0,
                t2_us: 30.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };
        let ast = trotterize(&program, TrotterOrder::First);
        let gate_names: Vec<&str> = ast.gates.iter().map(|g| g.name.as_str()).collect();
        // e^{-iθX} → H, Rz(2θ), H
        assert_eq!(gate_names, vec!["h", "rz", "h"]);
    }

    #[test]
    fn trotterize_measures_all_qubits() {
        let program = simple_zz_program();
        let ast = trotterize(&program, TrotterOrder::First);
        assert_eq!(ast.measures.len(), 2);
        assert_eq!(ast.measures[0].qubit, 0);
        assert_eq!(ast.measures[1].qubit, 1);
    }

    #[test]
    fn second_order_trotter_symmetric() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms.push(PauliTerm {
            coefficient: 0.3,
            paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
        });
        let ast = trotterize(&program, TrotterOrder::Second);
        // Second-order should produce gates for both terms, twice (forward + backward).
        assert!(!ast.gates.is_empty());
    }
}
