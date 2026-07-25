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
//!
//! Supported orders:
//! - **First-order**: O(dt²) error per step, O(dt) global error.
//! - **Second-order**: symmetric Suzuki, O(dt³) error per step, O(dt²) global.
//! - **Fourth-order**: recursive Suzuki (S₄), O(dt⁵) error per step, O(dt⁴) global.

use thiserror::Error;

use crate::ast::*;
use crate::cbor::{EhrenfestProgram, NoiseConstraint, PauliOp, PauliOpEntry};

// ── Gate timing constants (same as noise.rs) ────────────────────────────────

const SINGLE_QUBIT_GATE_TIME_US: f64 = 0.05;
const TWO_QUBIT_GATE_TIME_US: f64 = 0.5;

// ── Errors ──────────────────────────────────────────────────────────────────

/// Errors raised during Hamiltonian validation.
#[derive(Debug, Error)]
pub enum TrotterError {
    #[error("empty Hamiltonian: at least one Pauli term is required")]
    EmptyHamiltonian,

    #[error("non-finite coefficient {coefficient} in Hamiltonian term {term_index}")]
    NonFiniteCoefficient { term_index: usize, coefficient: f64 },

    #[error("qubit index {qubit} in Hamiltonian term {term_index} out of range (n_qubits={n_qubits})")]
    QubitOutOfRange {
        term_index: usize,
        qubit: usize,
        n_qubits: usize,
    },

    #[error("non-finite evolution parameter: dt_us={dt_us}, total_us={total_us}")]
    NonFiniteEvolution { dt_us: f64, total_us: f64 },

    #[error("zero Trotter steps")]
    ZeroSteps,
}

// ── Public types ────────────────────────────────────────────────────────────

/// Trotter decomposition order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrotterOrder {
    #[default]
    First,
    Second,
    Fourth,
}

/// Statistics from a Trotterization pass.
#[derive(Debug, Clone)]
pub struct TrotterStats {
    /// Trotter order used.
    pub order: TrotterOrder,
    /// Number of Trotter time steps.
    pub steps: u32,
    /// Time step size in microseconds.
    pub dt_us: f64,
    /// Number of Hamiltonian terms.
    pub hamiltonian_terms: usize,
    /// Total gates generated.
    pub total_gates: usize,
    /// Number of single-qubit gates generated.
    pub single_qubit_gates: usize,
    /// Number of two-qubit (CX) gates generated.
    pub two_qubit_gates: usize,
    /// Estimated Trotter approximation error bound.
    pub error_bound: f64,
    /// Estimated circuit execution time in microseconds.
    pub estimated_time_us: f64,
}

/// Result of a noise-budget check against a Trotterized circuit.
#[derive(Debug, Clone)]
pub struct NoiseBudgetReport {
    /// Whether the circuit fits within T1 relaxation time.
    pub within_t1: bool,
    /// Whether the circuit fits within T2 coherence time.
    pub within_t2: bool,
    /// Whether the estimated fidelity meets the minimum gate fidelity.
    pub fidelity_ok: bool,
    /// Estimated circuit execution time in microseconds.
    pub estimated_time_us: f64,
    /// Estimated circuit fidelity.
    pub estimated_fidelity: f64,
    /// Warnings about noise budget violations.
    pub warnings: Vec<String>,
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Validate an Ehrenfest program's Hamiltonian before Trotterization.
///
/// Checks:
/// - Hamiltonian has at least one term
/// - All coefficients are finite
/// - All qubit indices are within `n_qubits`
/// - Evolution parameters are finite and non-zero
pub fn validate_hamiltonian(program: &EhrenfestProgram) -> Result<(), TrotterError> {
    if program.hamiltonian.terms.is_empty() {
        return Err(TrotterError::EmptyHamiltonian);
    }

    if program.evolution.steps == 0 {
        return Err(TrotterError::ZeroSteps);
    }

    if !program.evolution.dt_us.is_finite() || !program.evolution.total_us.is_finite() {
        return Err(TrotterError::NonFiniteEvolution {
            dt_us: program.evolution.dt_us,
            total_us: program.evolution.total_us,
        });
    }

    let n_qubits = program.system.n_qubits;
    for (i, term) in program.hamiltonian.terms.iter().enumerate() {
        if !term.coefficient.is_finite() {
            return Err(TrotterError::NonFiniteCoefficient {
                term_index: i,
                coefficient: term.coefficient,
            });
        }
        for entry in &term.paulis {
            if entry.qubit >= n_qubits {
                return Err(TrotterError::QubitOutOfRange {
                    term_index: i,
                    qubit: entry.qubit,
                    n_qubits,
                });
            }
        }
    }

    Ok(())
}

// ── Trotter error estimation ────────────────────────────────────────────────

/// Estimate the Trotter approximation error bound.
///
/// For a Hamiltonian H = Σ Hₖ with L terms, the error per step is:
/// - First-order:  ε ≤ L² · max|\[Hⱼ,Hₖ\]| · dt² / 2
/// - Second-order: ε ≤ L³ · max|\[\[Hⱼ,Hₖ\],Hₗ\]| · dt³ / 12
/// - Fourth-order: ε ≤ C · L⁵ · ||H||⁵ · dt⁵
///
/// We use the simplified bound based on the sum of absolute coefficients
/// as a proxy for ||H||, since exact commutator norms are expensive.
pub fn estimate_trotter_error(program: &EhrenfestProgram, order: TrotterOrder) -> f64 {
    let n_terms = program.hamiltonian.terms.len() as f64;
    let dt = program.evolution.dt_us;
    let steps = program.evolution.steps as f64;

    // Hamiltonian norm proxy: sum of absolute coefficients.
    let h_norm: f64 = program
        .hamiltonian
        .terms
        .iter()
        .map(|t| t.coefficient.abs())
        .sum();

    if h_norm == 0.0 || n_terms == 0.0 {
        return 0.0;
    }

    // Error per step, then multiply by number of steps for global bound.
    let per_step = match order {
        TrotterOrder::First => n_terms * n_terms * h_norm * h_norm * dt * dt / 2.0,
        TrotterOrder::Second => {
            n_terms.powi(3) * h_norm.powi(3) * dt.powi(3) / 12.0
        }
        TrotterOrder::Fourth => {
            // Fourth-order Suzuki: error scales as dt^5.
            // Prefactor from recursive construction.
            n_terms.powi(5) * h_norm.powi(5) * dt.powi(5) / 720.0
        }
    };

    per_step * steps
}

// ── Trotterization ──────────────────────────────────────────────────────────

/// Derive a gate-level AST from an Ehrenfest physics program via Trotterization.
///
/// The `order` parameter controls the decomposition:
/// - First-order: e^{-iH₁dt} · e^{-iH₂dt} · ...
/// - Second-order: symmetric Suzuki-Trotter (forward half-step + reverse half-step)
/// - Fourth-order: recursive Suzuki (two time-scales)
pub fn trotterize(program: &EhrenfestProgram, order: TrotterOrder) -> EhrenfestAst {
    let dt = program.evolution.dt_us;
    let mut gates = Vec::new();

    for _step in 0..program.evolution.steps {
        match order {
            TrotterOrder::First => {
                trotter_first_order(&program.hamiltonian.terms, dt, &mut gates);
            }
            TrotterOrder::Second => {
                trotter_second_order(&program.hamiltonian.terms, dt, &mut gates);
            }
            TrotterOrder::Fourth => {
                trotter_fourth_order(&program.hamiltonian.terms, dt, &mut gates);
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

/// Trotterize with validation and statistics.
///
/// Validates the Hamiltonian, performs Trotterization, and returns both
/// the circuit AST and compilation statistics.
pub fn trotterize_with_stats(
    program: &EhrenfestProgram,
    order: TrotterOrder,
) -> Result<(EhrenfestAst, TrotterStats), TrotterError> {
    validate_hamiltonian(program)?;

    let ast = trotterize(program, order);

    let single_qubit_gates = ast.gates.iter().filter(|g| g.name.arity() == 1).count();
    let two_qubit_gates = ast.gates.iter().filter(|g| g.name.arity() >= 2).count();

    let estimated_time_us = single_qubit_gates as f64 * SINGLE_QUBIT_GATE_TIME_US
        + two_qubit_gates as f64 * TWO_QUBIT_GATE_TIME_US;

    let stats = TrotterStats {
        order,
        steps: program.evolution.steps,
        dt_us: program.evolution.dt_us,
        hamiltonian_terms: program.hamiltonian.terms.len(),
        total_gates: ast.gates.len(),
        single_qubit_gates,
        two_qubit_gates,
        error_bound: estimate_trotter_error(program, order),
        estimated_time_us,
    };

    Ok((ast, stats))
}

// ── Noise budget ────────────────────────────────────────────────────────────

/// Check whether a Trotterized circuit fits within the noise budget.
///
/// Compares the estimated circuit execution time and fidelity against the
/// noise constraints from the Ehrenfest program.
pub fn check_noise_budget(stats: &TrotterStats, noise: &NoiseConstraint) -> NoiseBudgetReport {
    let estimated_time_us = stats.estimated_time_us;
    let mut warnings = Vec::new();

    let within_t1 = estimated_time_us <= noise.t1_us;
    let within_t2 = estimated_time_us <= noise.t2_us;

    if !within_t2 {
        warnings.push(format!(
            "circuit time ({:.2} us) exceeds T2 coherence ({:.2} us) — \
             consider increasing Trotter steps to reduce dt",
            estimated_time_us, noise.t2_us,
        ));
    }

    if !within_t1 {
        warnings.push(format!(
            "circuit time ({:.2} us) exceeds T1 relaxation ({:.2} us)",
            estimated_time_us, noise.t1_us,
        ));
    }

    // Fidelity estimation: (1-ε₁)^n₁ · (1-ε₂)^n₂
    let single_error = 0.001_f64;
    let two_error = 0.01_f64;
    let estimated_fidelity = (1.0 - single_error).powi(stats.single_qubit_gates as i32)
        * (1.0 - two_error).powi(stats.two_qubit_gates as i32);

    let fidelity_ok = noise
        .gate_fidelity_min
        .is_none_or(|threshold| estimated_fidelity >= threshold);

    if !fidelity_ok {
        if let Some(threshold) = noise.gate_fidelity_min {
            warnings.push(format!(
                "estimated fidelity ({:.6}) below threshold ({:.6})",
                estimated_fidelity, threshold,
            ));
        }
    }

    NoiseBudgetReport {
        within_t1,
        within_t2,
        fidelity_ok,
        estimated_time_us,
        estimated_fidelity,
        warnings,
    }
}

/// Format Trotter statistics for human-readable output.
pub fn format_trotter_stats(stats: &TrotterStats) -> String {
    let order_str = match stats.order {
        TrotterOrder::First => "1st",
        TrotterOrder::Second => "2nd",
        TrotterOrder::Fourth => "4th",
    };
    let mut lines = Vec::new();
    lines.push(format!("  Trotter order:      {order_str}"));
    lines.push(format!("  Trotter steps:      {}", stats.steps));
    lines.push(format!("  dt:                 {:.4} us", stats.dt_us));
    lines.push(format!("  Hamiltonian terms:  {}", stats.hamiltonian_terms));
    lines.push(format!("  Total gates:        {}", stats.total_gates));
    lines.push(format!("  1Q gates:           {}", stats.single_qubit_gates));
    lines.push(format!("  2Q gates:           {}", stats.two_qubit_gates));
    lines.push(format!("  Error bound:        {:.2e}", stats.error_bound));
    lines.push(format!(
        "  Est. circuit time:  {:.2} us",
        stats.estimated_time_us
    ));
    lines.join("\n")
}

// ── Trotter step implementations ────────────────────────────────────────────

/// First-order Trotter step: e^{-iH₁dt} · e^{-iH₂dt} · ...
fn trotter_first_order(
    terms: &[crate::cbor::PauliTerm],
    dt: f64,
    gates: &mut Vec<Gate>,
) {
    for term in terms {
        let theta = term.coefficient * dt;
        gates.extend(pauli_rotation_gates(&term.paulis, theta));
    }
}

/// Second-order Trotter step (symmetric Suzuki):
///   S₂(dt) = Π_k e^{-iHₖ dt/2} · Π_k' e^{-iHₖ' dt/2}  (reverse order)
fn trotter_second_order(
    terms: &[crate::cbor::PauliTerm],
    dt: f64,
    gates: &mut Vec<Gate>,
) {
    let half_dt = dt / 2.0;
    // Forward half-step
    for term in terms {
        let theta = term.coefficient * half_dt;
        gates.extend(pauli_rotation_gates(&term.paulis, theta));
    }
    // Backward half-step (reverse order)
    for term in terms.iter().rev() {
        let theta = term.coefficient * half_dt;
        gates.extend(pauli_rotation_gates(&term.paulis, theta));
    }
}

/// Fourth-order Suzuki-Trotter step:
///   S₄(dt) = S₂(p·dt) · S₂(p·dt) · S₂((1-4p)·dt) · S₂(p·dt) · S₂(p·dt)
///
/// where p = 1 / (4 - 4^{1/3}).
fn trotter_fourth_order(
    terms: &[crate::cbor::PauliTerm],
    dt: f64,
    gates: &mut Vec<Gate>,
) {
    // Suzuki fractal scaling constant.
    let p = 1.0 / (4.0 - 4.0_f64.cbrt());
    let p_dt = p * dt;
    let mid_dt = (1.0 - 4.0 * p) * dt;

    trotter_second_order(terms, p_dt, gates);
    trotter_second_order(terms, p_dt, gates);
    trotter_second_order(terms, mid_dt, gates);
    trotter_second_order(terms, p_dt, gates);
    trotter_second_order(terms, p_dt, gates);
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

    fn two_term_program() -> EhrenfestProgram {
        EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 2,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![
                    PauliTerm {
                        coefficient: 0.5,
                        paulis: vec![
                            PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                            PauliOpEntry { qubit: 1, axis: PauliOp::Z },
                        ],
                    },
                    PauliTerm {
                        coefficient: 0.3,
                        paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
                    },
                ],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 10,
                dt_us: 0.1,
            },
            observables: vec![Observable::SZ { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: Some(0.99),
                readout_fidelity_min: None,
            },
        }
    }

    // ── Basic trotterization tests ───────────────────────────────────────

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

    // ── Fourth-order Trotter tests ───────────────────────────────────────

    #[test]
    fn fourth_order_produces_more_gates_than_second() {
        let program = two_term_program();
        let ast2 = trotterize(&program, TrotterOrder::Second);
        let ast4 = trotterize(&program, TrotterOrder::Fourth);
        // Fourth-order uses 5 second-order sub-steps per Trotter step,
        // so it should produce roughly 5× the gates.
        assert!(
            ast4.gates.len() > ast2.gates.len(),
            "4th-order ({}) should produce more gates than 2nd-order ({})",
            ast4.gates.len(),
            ast2.gates.len(),
        );
    }

    #[test]
    fn fourth_order_gate_count_ratio() {
        let program = two_term_program();
        let ast2 = trotterize(&program, TrotterOrder::Second);
        let ast4 = trotterize(&program, TrotterOrder::Fourth);
        // S4 = 5 × S2 sub-steps, so gate ratio should be exactly 5.
        let ratio = ast4.gates.len() as f64 / ast2.gates.len() as f64;
        assert!(
            (ratio - 5.0).abs() < 1e-10,
            "4th/2nd gate ratio should be 5.0, got {ratio}",
        );
    }

    #[test]
    fn fourth_order_single_step_structure() {
        // Single step, single term: verify S4 = 5 × S2.
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
            observables: vec![],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };
        let ast2 = trotterize(&program, TrotterOrder::Second);
        let ast4 = trotterize(&program, TrotterOrder::Fourth);
        // X term: S2 produces 2 × (H, Rz, H) = 6 gates.
        assert_eq!(ast2.gates.len(), 6);
        // S4 = 5 × S2 = 30 gates.
        assert_eq!(ast4.gates.len(), 30);
    }

    // ── Validation tests ─────────────────────────────────────────────────

    #[test]
    fn validate_good_program() {
        let program = simple_zz_program();
        assert!(validate_hamiltonian(&program).is_ok());
    }

    #[test]
    fn validate_rejects_empty_hamiltonian() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms.clear();
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::EmptyHamiltonian));
    }

    #[test]
    fn validate_rejects_nan_coefficient() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms[0].coefficient = f64::NAN;
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::NonFiniteCoefficient { .. }));
    }

    #[test]
    fn validate_rejects_infinite_coefficient() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms[0].coefficient = f64::INFINITY;
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::NonFiniteCoefficient { .. }));
    }

    #[test]
    fn validate_rejects_qubit_out_of_range() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms[0].paulis[0].qubit = 99;
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::QubitOutOfRange { .. }));
    }

    #[test]
    fn validate_rejects_zero_steps() {
        let mut program = simple_zz_program();
        program.evolution.steps = 0;
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::ZeroSteps));
    }

    #[test]
    fn validate_rejects_nan_evolution() {
        let mut program = simple_zz_program();
        program.evolution.dt_us = f64::NAN;
        let err = validate_hamiltonian(&program).unwrap_err();
        assert!(matches!(err, TrotterError::NonFiniteEvolution { .. }));
    }

    // ── Error estimation tests ───────────────────────────────────────────

    #[test]
    fn error_bound_decreases_with_order() {
        let program = two_term_program();
        let e1 = estimate_trotter_error(&program, TrotterOrder::First);
        let e2 = estimate_trotter_error(&program, TrotterOrder::Second);
        let e4 = estimate_trotter_error(&program, TrotterOrder::Fourth);
        // Higher order should have smaller error for small dt.
        assert!(
            e2 < e1,
            "2nd-order error ({e2:.2e}) should be < 1st-order ({e1:.2e})"
        );
        assert!(
            e4 < e2,
            "4th-order error ({e4:.2e}) should be < 2nd-order ({e2:.2e})"
        );
    }

    #[test]
    fn error_bound_zero_for_zero_hamiltonian() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms[0].coefficient = 0.0;
        assert_eq!(estimate_trotter_error(&program, TrotterOrder::First), 0.0);
    }

    #[test]
    fn error_bound_scales_with_steps() {
        let mut program = two_term_program();
        let e10 = estimate_trotter_error(&program, TrotterOrder::First);
        program.evolution.steps = 20;
        program.evolution.dt_us = program.evolution.total_us / 20.0;
        let e20 = estimate_trotter_error(&program, TrotterOrder::First);
        // Doubling steps halves dt, so per-step error ∝ dt² goes to 1/4,
        // but total error = per-step × steps, so overall goes to 1/2.
        assert!(
            e20 < e10,
            "more steps should reduce error: {e20:.2e} vs {e10:.2e}"
        );
    }

    #[test]
    fn error_bound_positive_for_nonzero_hamiltonian() {
        let program = two_term_program();
        let e = estimate_trotter_error(&program, TrotterOrder::First);
        assert!(e > 0.0, "error bound should be positive, got {e}");
    }

    // ── Statistics tests ─────────────────────────────────────────────────

    #[test]
    fn trotterize_with_stats_returns_correct_counts() {
        let program = two_term_program();
        let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
        assert_eq!(stats.order, TrotterOrder::First);
        assert_eq!(stats.steps, 10);
        assert_eq!(stats.hamiltonian_terms, 2);
        assert_eq!(stats.total_gates, ast.gates.len());
        assert_eq!(
            stats.single_qubit_gates + stats.two_qubit_gates,
            stats.total_gates
        );
        assert!(stats.estimated_time_us > 0.0);
        assert!(stats.error_bound > 0.0);
    }

    #[test]
    fn trotterize_with_stats_rejects_invalid() {
        let mut program = simple_zz_program();
        program.hamiltonian.terms.clear();
        assert!(trotterize_with_stats(&program, TrotterOrder::First).is_err());
    }

    #[test]
    fn format_stats_contains_all_fields() {
        let program = two_term_program();
        let (_, stats) = trotterize_with_stats(&program, TrotterOrder::Second).unwrap();
        let formatted = format_trotter_stats(&stats);
        assert!(formatted.contains("2nd"));
        assert!(formatted.contains("Trotter steps"));
        assert!(formatted.contains("Hamiltonian terms"));
        assert!(formatted.contains("Total gates"));
        assert!(formatted.contains("Error bound"));
    }

    // ── Noise budget tests ───────────────────────────────────────────────

    #[test]
    fn noise_budget_within_limits() {
        let program = simple_zz_program();
        let (_, stats) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
        let report = check_noise_budget(&stats, &program.noise);
        assert!(report.within_t1);
        assert!(report.within_t2);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn noise_budget_detects_t2_violation() {
        let program = two_term_program();
        let (_, stats) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
        let tight_noise = NoiseConstraint {
            t1_us: 100.0,
            t2_us: 0.01, // Very short T2
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        };
        let report = check_noise_budget(&stats, &tight_noise);
        assert!(!report.within_t2);
        assert!(!report.warnings.is_empty());
        assert!(report.warnings[0].contains("T2"));
    }

    #[test]
    fn noise_budget_detects_fidelity_violation() {
        let program = two_term_program();
        let (_, stats) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
        let strict_noise = NoiseConstraint {
            t1_us: 10000.0,
            t2_us: 10000.0,
            gate_fidelity_min: Some(0.9999), // Very strict
            readout_fidelity_min: None,
        };
        let report = check_noise_budget(&stats, &strict_noise);
        assert!(!report.fidelity_ok);
        assert!(report.warnings.iter().any(|w| w.contains("fidelity")));
    }

    // ── Display rendering tests ───────────────────────────────────────────

    #[test]
    fn empty_hamiltonian_renders_message() {
        let err = TrotterError::EmptyHamiltonian;
        assert_eq!(
            err.to_string(),
            "empty Hamiltonian: at least one Pauli term is required"
        );
    }

    #[test]
    fn non_finite_coefficient_renders_term_and_coefficient() {
        let err = TrotterError::NonFiniteCoefficient {
            term_index: 7,
            coefficient: f64::NAN,
        };
        assert_eq!(
            err.to_string(),
            "non-finite coefficient NaN in Hamiltonian term 7"
        );
    }

    #[test]
    fn qubit_out_of_range_renders_indices() {
        let err = TrotterError::QubitOutOfRange {
            term_index: 2,
            qubit: 9,
            n_qubits: 4,
        };
        assert_eq!(
            err.to_string(),
            "qubit index 9 in Hamiltonian term 2 out of range (n_qubits=4)"
        );
    }

    #[test]
    fn non_finite_evolution_renders_parameters() {
        let err = TrotterError::NonFiniteEvolution {
            dt_us: f64::INFINITY,
            total_us: f64::NAN,
        };
        assert_eq!(
            err.to_string(),
            "non-finite evolution parameter: dt_us=inf, total_us=NaN"
        );
    }

    #[test]
    fn zero_steps_renders_message() {
        let err = TrotterError::ZeroSteps;
        assert_eq!(err.to_string(), "zero Trotter steps");
    }
}
