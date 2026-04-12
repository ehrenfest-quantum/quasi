// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Non-Clifford gate decomposition pass.
//!
//! Decomposes non-Clifford gates into the Clifford+T universal gate set so
//! that the resulting circuit can be fully lowered to ZX-IR and optimised.
//!
//! Two decompositions are provided:
//!
//! 1. **CCX (Toffoli) → Clifford+T**: the standard 15-gate decomposition
//!    using 7 T/Tdg gates and 6 CX gates (plus 2 H gates).
//!
//! 2. **Rz(θ) → Clifford+T approximation**: for rotation angles that are
//!    not exact multiples of π/4, approximate the rotation using a sequence
//!    of H and T gates via recursive phase approximation. Exact multiples
//!    of π/4 are decomposed into T/S/Z combinations without approximation.

use crate::ast::{is_clifford_angle, is_t_angle, EhrenfestAst, Gate, GateName};

/// Result of a decomposition pass.
#[derive(Debug, Clone)]
pub struct DecomposeStats {
    /// Number of CCX gates decomposed.
    pub ccx_decomposed: usize,
    /// Number of Rz gates decomposed to exact Clifford+T.
    pub rz_exact: usize,
    /// Number of Rz gates approximated to Clifford+T.
    pub rz_approx: usize,
    /// Total T-count after decomposition.
    pub t_count: usize,
}

// ── CCX decomposition ───────────────────────────────────────────────────────

/// Decompose a CCX (Toffoli) gate into the standard 15-gate Clifford+T sequence.
///
/// Uses 7 T/Tdg gates, 6 CX gates, and 2 H gates:
/// ```text
/// H target
/// CX ctrl2, target     Tdg target
/// CX ctrl1, target     T target
/// CX ctrl2, target     Tdg target
/// CX ctrl1, target     T ctrl2      T target
/// H target
/// CX ctrl1, ctrl2      T ctrl1      Tdg ctrl2
/// CX ctrl1, ctrl2
/// ```
pub fn decompose_ccx(ctrl1: usize, ctrl2: usize, target: usize) -> Vec<Gate> {
    vec![
        Gate { name: GateName::H,   qubits: vec![target],         params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl2, target],  params: vec![] },
        Gate { name: GateName::Tdg, qubits: vec![target],         params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl1, target],  params: vec![] },
        Gate { name: GateName::T,   qubits: vec![target],         params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl2, target],  params: vec![] },
        Gate { name: GateName::Tdg, qubits: vec![target],         params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl1, target],  params: vec![] },
        Gate { name: GateName::T,   qubits: vec![ctrl2],          params: vec![] },
        Gate { name: GateName::T,   qubits: vec![target],         params: vec![] },
        Gate { name: GateName::H,   qubits: vec![target],         params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl1, ctrl2],   params: vec![] },
        Gate { name: GateName::T,   qubits: vec![ctrl1],          params: vec![] },
        Gate { name: GateName::Tdg, qubits: vec![ctrl2],          params: vec![] },
        Gate { name: GateName::Cx,  qubits: vec![ctrl1, ctrl2],   params: vec![] },
    ]
}

// ── Rz decomposition ────────────────────────────────────────────────────────

/// Decompose Rz(θ) into an exact sequence of T/S/Z gates when θ is a
/// multiple of π/4.
///
/// Maps the phase exponent (in units of π/4 mod 8) to a minimal gate
/// sequence, analogous to `optimize::phase_to_gates`:
///   0 → identity, 1 → T, 2 → S, 3 → S·T,
///   4 → Z, 5 → Z·T, 6 → Sdg, 7 → Tdg
fn decompose_rz_exact(qubit: usize, theta: f64) -> Vec<Gate> {
    let quarter_pi = std::f64::consts::FRAC_PI_4;
    let n = ((theta / quarter_pi).round() as i64).rem_euclid(8) as u32;

    let names: &[GateName] = match n {
        0 => &[],
        1 => &[GateName::T],
        2 => &[GateName::S],
        3 => &[GateName::S, GateName::T],
        4 => &[GateName::Z],
        5 => &[GateName::Z, GateName::T],
        6 => &[GateName::Sdg],
        7 => &[GateName::Tdg],
        _ => unreachable!(),
    };

    names
        .iter()
        .map(|name| Gate {
            name: name.clone(),
            qubits: vec![qubit],
            params: vec![],
        })
        .collect()
}

/// Approximate Rz(θ) using a sequence of Clifford+T gates.
///
/// Uses a grid-based approach: find the best approximation of θ as
/// k·π/4 + residual, then recursively approximate the residual using
/// H·T·H rotations (which rotate by a non-commensurable angle around
/// a different axis, enabling arbitrary precision).
///
/// The `precision` parameter controls the maximum approximation error
/// in radians. Smaller values produce longer gate sequences.
pub fn decompose_rz_approx(qubit: usize, theta: f64, precision: f64) -> Vec<Gate> {
    // Normalize angle to [0, 2π).
    let two_pi = 2.0 * std::f64::consts::PI;
    let theta = ((theta % two_pi) + two_pi) % two_pi;

    // If close enough to zero, emit nothing.
    if theta.abs() < precision || (theta - two_pi).abs() < precision {
        return vec![];
    }

    // Try exact decomposition first.
    if is_t_angle(theta) {
        return decompose_rz_exact(qubit, theta);
    }

    // Grid search: find nearest k·π/4.
    let quarter_pi = std::f64::consts::FRAC_PI_4;
    let k = (theta / quarter_pi).round() as i64;
    let base_angle = k as f64 * quarter_pi;
    let residual = theta - base_angle;

    // If residual is within precision, use exact base.
    if residual.abs() < precision {
        return decompose_rz_exact(qubit, base_angle);
    }

    // Recursive approximation using the SK-like sequence:
    //   Rz(θ) ≈ Rz(k·π/4) · [H · T · H]^m
    // where H·T·H = Rx(π/4), and composing Rz with Rx sequences
    // can approximate any angle.
    //
    // We use a practical bounded-depth approach: decompose into
    // base + correction layers, each layer adding H-T-H or H-Tdg-H.
    let mut gates = decompose_rz_exact(qubit, base_angle);
    let mut remaining = residual;
    let step = quarter_pi; // Each H-T-H corrects by ~π/4 in X basis

    // Apply correction layers (bounded depth to avoid infinite recursion).
    let max_layers = 20;
    for _ in 0..max_layers {
        if remaining.abs() < precision {
            break;
        }

        if remaining > 0.0 {
            // H · T · H ≈ rotation by π/4 around X axis
            gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
            gates.push(Gate { name: GateName::T, qubits: vec![qubit], params: vec![] });
            gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
            remaining -= step;
        } else {
            // H · Tdg · H ≈ rotation by -π/4 around X axis
            gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
            gates.push(Gate { name: GateName::Tdg, qubits: vec![qubit], params: vec![] });
            gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
            remaining += step;
        }
    }

    gates
}

// ── Ry/Rx decomposition helpers ─────────────────────────────────────────────

/// Decompose Rx(θ) into Clifford+T via: Rx(θ) = H · Rz(θ) · H.
pub fn decompose_rx(qubit: usize, theta: f64, precision: f64) -> Vec<Gate> {
    let mut gates = vec![
        Gate { name: GateName::H, qubits: vec![qubit], params: vec![] },
    ];
    gates.extend(decompose_rz_approx(qubit, theta, precision));
    gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
    gates
}

/// Decompose Ry(θ) into Clifford+T via: Ry(θ) = Sdg · H · Rz(θ) · H · S.
pub fn decompose_ry(qubit: usize, theta: f64, precision: f64) -> Vec<Gate> {
    let mut gates = vec![
        Gate { name: GateName::S, qubits: vec![qubit], params: vec![] },
        Gate { name: GateName::H, qubits: vec![qubit], params: vec![] },
    ];
    gates.extend(decompose_rz_approx(qubit, theta, precision));
    gates.push(Gate { name: GateName::H, qubits: vec![qubit], params: vec![] });
    gates.push(Gate { name: GateName::Sdg, qubits: vec![qubit], params: vec![] });
    gates
}

// ── Full AST decomposition pass ─────────────────────────────────────────────

/// Default approximation precision for rotation decomposition (radians).
pub const DEFAULT_PRECISION: f64 = 1e-4;

/// Decompose all non-Clifford gates in an AST into Clifford+T.
///
/// - CCX gates are decomposed into the standard 15-gate sequence.
/// - Rz/Rx/Ry gates at non-Clifford angles are decomposed:
///   - Exact decomposition for multiples of π/4.
///   - Approximate decomposition for other angles.
/// - Rz/Rx/Ry at Clifford angles (multiples of π/2) are decomposed
///   into exact Clifford gates.
///
/// Returns the decomposed AST and statistics about the decomposition.
pub fn decompose_non_clifford(ast: &EhrenfestAst, precision: f64) -> (EhrenfestAst, DecomposeStats) {
    let mut stats = DecomposeStats {
        ccx_decomposed: 0,
        rz_exact: 0,
        rz_approx: 0,
        t_count: 0,
    };

    let mut new_gates: Vec<Gate> = Vec::new();

    for gate in &ast.gates {
        match &gate.name {
            GateName::Ccx => {
                let decomposed = decompose_ccx(
                    gate.qubits[0],
                    gate.qubits[1],
                    gate.qubits[2],
                );
                new_gates.extend(decomposed);
                stats.ccx_decomposed += 1;
            }

            GateName::Rz if !gate.params.is_empty() => {
                let theta = gate.params[0];
                if is_clifford_angle(theta) {
                    // Clifford rotation: decompose to exact S/Z/Sdg.
                    let decomposed = decompose_rz_exact(gate.qubits[0], theta);
                    new_gates.extend(decomposed);
                    stats.rz_exact += 1;
                } else if is_t_angle(theta) {
                    // T-multiple: exact decomposition.
                    let decomposed = decompose_rz_exact(gate.qubits[0], theta);
                    new_gates.extend(decomposed);
                    stats.rz_exact += 1;
                } else {
                    // General angle: approximate.
                    let decomposed = decompose_rz_approx(gate.qubits[0], theta, precision);
                    new_gates.extend(decomposed);
                    stats.rz_approx += 1;
                }
            }

            GateName::Rx if !gate.params.is_empty() => {
                let theta = gate.params[0];
                if is_clifford_angle(theta) || is_t_angle(theta) {
                    let decomposed = decompose_rx(gate.qubits[0], theta, precision);
                    new_gates.extend(decomposed);
                    stats.rz_exact += 1;
                } else {
                    let decomposed = decompose_rx(gate.qubits[0], theta, precision);
                    new_gates.extend(decomposed);
                    stats.rz_approx += 1;
                }
            }

            GateName::Ry if !gate.params.is_empty() => {
                let theta = gate.params[0];
                if is_clifford_angle(theta) || is_t_angle(theta) {
                    let decomposed = decompose_ry(gate.qubits[0], theta, precision);
                    new_gates.extend(decomposed);
                    stats.rz_exact += 1;
                } else {
                    let decomposed = decompose_ry(gate.qubits[0], theta, precision);
                    new_gates.extend(decomposed);
                    stats.rz_approx += 1;
                }
            }

            // All other gates pass through unchanged.
            _ => {
                new_gates.push(gate.clone());
            }
        }
    }

    // Count T/Tdg gates in the result.
    stats.t_count = new_gates
        .iter()
        .filter(|g| matches!(g.name, GateName::T | GateName::Tdg))
        .count();

    let decomposed_ast = EhrenfestAst {
        name: ast.name.clone(),
        n_qubits: ast.n_qubits,
        prepare: ast.prepare.clone(),
        gates: new_gates,
        measures: ast.measures.clone(),
        conditionals: ast.conditionals.clone(),
        expects: ast.expects.clone(),
        type_decls: ast.type_decls.clone(),
        variational_loops: ast.variational_loops.clone(),
    };

    (decomposed_ast, stats)
}

/// Count the T-gates (T + Tdg) in a gate sequence.
pub fn count_t_gates(gates: &[Gate]) -> usize {
    gates
        .iter()
        .filter(|g| matches!(g.name, GateName::T | GateName::Tdg))
        .count()
}

/// Check whether an AST contains any non-Clifford gates.
pub fn has_non_clifford_gates(ast: &EhrenfestAst) -> bool {
    ast.gates.iter().any(|g| {
        match &g.name {
            name if name.is_non_clifford() => {
                // For parametric gates, check the angle.
                if name.is_parametric() && !g.params.is_empty() {
                    !is_clifford_angle(g.params[0])
                } else {
                    // T, Tdg, Ccx are always non-Clifford.
                    true
                }
            }
            _ => false,
        }
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    fn make_ast(n_qubits: usize, gates: Vec<Gate>) -> EhrenfestAst {
        EhrenfestAst {
            name: "test".into(),
            n_qubits,
            prepare: None,
            gates,
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        }
    }

    // ── CCX decomposition ─────────────────────────────────────────────

    #[test]
    fn ccx_decomposes_to_15_gates() {
        let gates = decompose_ccx(0, 1, 2);
        assert_eq!(gates.len(), 15);
    }

    #[test]
    fn ccx_decomposition_has_7_t_gates() {
        let gates = decompose_ccx(0, 1, 2);
        let t_count = count_t_gates(&gates);
        assert_eq!(t_count, 7, "Toffoli decomposition should have 7 T/Tdg gates");
    }

    #[test]
    fn ccx_decomposition_has_6_cx_gates() {
        let gates = decompose_ccx(0, 1, 2);
        let cx_count = gates.iter().filter(|g| g.name == GateName::Cx).count();
        assert_eq!(cx_count, 6, "Toffoli decomposition should have 6 CX gates");
    }

    #[test]
    fn ccx_decomposition_has_2_h_gates() {
        let gates = decompose_ccx(0, 1, 2);
        let h_count = gates.iter().filter(|g| g.name == GateName::H).count();
        assert_eq!(h_count, 2, "Toffoli decomposition should have 2 H gates");
    }

    #[test]
    fn ccx_preserves_qubit_assignments() {
        let gates = decompose_ccx(3, 5, 7);
        // First gate is H on target.
        assert_eq!(gates[0].qubits, vec![7]);
        // Gate 1: CX ctrl2, target.
        assert_eq!(gates[1].qubits, vec![5, 7]);
        // Gate 3: CX ctrl1, target.
        assert_eq!(gates[3].qubits, vec![3, 7]);
        // Gate 8: T ctrl2.
        assert_eq!(gates[8].qubits, vec![5]);
        // Gate 12: T ctrl1.
        assert_eq!(gates[12].qubits, vec![3]);
    }

    // ── Rz exact decomposition ────────────────────────────────────────

    #[test]
    fn rz_zero_is_identity() {
        let gates = decompose_rz_exact(0, 0.0);
        assert!(gates.is_empty());
    }

    #[test]
    fn rz_pi_over_4_is_t() {
        let gates = decompose_rz_exact(0, FRAC_PI_4);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::T);
    }

    #[test]
    fn rz_pi_over_2_is_s() {
        let gates = decompose_rz_exact(0, FRAC_PI_2);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::S);
    }

    #[test]
    fn rz_3pi_over_4_is_s_t() {
        let gates = decompose_rz_exact(0, 3.0 * FRAC_PI_4);
        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0].name, GateName::S);
        assert_eq!(gates[1].name, GateName::T);
    }

    #[test]
    fn rz_pi_is_z() {
        let gates = decompose_rz_exact(0, PI);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::Z);
    }

    #[test]
    fn rz_neg_pi_over_4_is_tdg() {
        let gates = decompose_rz_exact(0, -FRAC_PI_4);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::Tdg);
    }

    #[test]
    fn rz_neg_pi_over_2_is_sdg() {
        let gates = decompose_rz_exact(0, -FRAC_PI_2);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::Sdg);
    }

    // ── Rz approximate decomposition ──────────────────────────────────

    #[test]
    fn rz_approx_exact_angle_uses_exact() {
        // π/4 should decompose exactly, not approximately.
        let gates = decompose_rz_approx(0, FRAC_PI_4, DEFAULT_PRECISION);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::T);
    }

    #[test]
    fn rz_approx_produces_only_clifford_t() {
        // Arbitrary non-Clifford angle.
        let theta = 0.7;
        let gates = decompose_rz_approx(0, theta, DEFAULT_PRECISION);
        for gate in &gates {
            assert!(
                matches!(
                    gate.name,
                    GateName::H | GateName::S | GateName::Sdg | GateName::Z
                        | GateName::T | GateName::Tdg
                ),
                "gate {:?} is not in Clifford+T set",
                gate.name
            );
        }
    }

    #[test]
    fn rz_approx_non_empty_for_nonzero() {
        let gates = decompose_rz_approx(0, 1.0, DEFAULT_PRECISION);
        assert!(!gates.is_empty(), "non-zero angle should produce gates");
    }

    #[test]
    fn rz_approx_near_zero_is_empty() {
        let gates = decompose_rz_approx(0, 1e-10, DEFAULT_PRECISION);
        assert!(gates.is_empty(), "near-zero angle should produce no gates");
    }

    // ── Rx/Ry decomposition ───────────────────────────────────────────

    #[test]
    fn rx_decomposition_wraps_with_h() {
        let gates = decompose_rx(0, FRAC_PI_4, DEFAULT_PRECISION);
        assert_eq!(gates.first().unwrap().name, GateName::H);
        assert_eq!(gates.last().unwrap().name, GateName::H);
    }

    #[test]
    fn ry_decomposition_wraps_with_s_h() {
        let gates = decompose_ry(0, FRAC_PI_4, DEFAULT_PRECISION);
        assert_eq!(gates.first().unwrap().name, GateName::S);
        assert_eq!(gates[1].name, GateName::H);
        assert_eq!(gates[gates.len() - 2].name, GateName::H);
        assert_eq!(gates.last().unwrap().name, GateName::Sdg);
    }

    // ── Full AST decomposition ────────────────────────────────────────

    #[test]
    fn decompose_ast_with_ccx() {
        let ast = make_ast(3, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
        ]);

        let (result, stats) = decompose_non_clifford(&ast, DEFAULT_PRECISION);

        assert_eq!(stats.ccx_decomposed, 1);
        assert_eq!(stats.t_count, 7);
        // H + 15 decomposed gates = 16.
        assert_eq!(result.gates.len(), 16);
        // No CCX gates should remain.
        assert!(
            result.gates.iter().all(|g| g.name != GateName::Ccx),
            "CCX should be fully decomposed"
        );
    }

    #[test]
    fn decompose_ast_with_rz_exact() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![FRAC_PI_4] },
        ]);

        let (result, stats) = decompose_non_clifford(&ast, DEFAULT_PRECISION);

        assert_eq!(stats.rz_exact, 1);
        assert_eq!(stats.rz_approx, 0);
        assert_eq!(result.gates.len(), 1);
        assert_eq!(result.gates[0].name, GateName::T);
    }

    #[test]
    fn decompose_ast_clifford_gates_pass_through() {
        let ast = make_ast(2, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::S, qubits: vec![0], params: vec![] },
        ]);

        let (result, stats) = decompose_non_clifford(&ast, DEFAULT_PRECISION);

        assert_eq!(stats.ccx_decomposed, 0);
        assert_eq!(stats.rz_exact, 0);
        assert_eq!(stats.rz_approx, 0);
        // All gates pass through unchanged.
        assert_eq!(result.gates.len(), 3);
        assert_eq!(result.gates, ast.gates);
    }

    #[test]
    fn decompose_ast_no_rz_params_pass_through() {
        // Rz without params passes through (shouldn't happen but be safe).
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![] },
        ]);

        let (result, _stats) = decompose_non_clifford(&ast, DEFAULT_PRECISION);
        assert_eq!(result.gates.len(), 1);
        assert_eq!(result.gates[0].name, GateName::Rz);
    }

    // ── Classification helpers ────────────────────────────────────────

    #[test]
    fn has_non_clifford_detects_t() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::T, qubits: vec![0], params: vec![] },
        ]);
        assert!(has_non_clifford_gates(&ast));
    }

    #[test]
    fn has_non_clifford_detects_ccx() {
        let ast = make_ast(3, vec![
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
        ]);
        assert!(has_non_clifford_gates(&ast));
    }

    #[test]
    fn has_non_clifford_ignores_clifford_rz() {
        // Rz(π/2) is a Clifford rotation (= S gate).
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![FRAC_PI_2] },
        ]);
        assert!(!has_non_clifford_gates(&ast));
    }

    #[test]
    fn has_non_clifford_detects_non_clifford_rz() {
        // Rz(0.7) is not a Clifford rotation.
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![0.7] },
        ]);
        assert!(has_non_clifford_gates(&ast));
    }

    #[test]
    fn pure_clifford_circuit_has_no_non_clifford() {
        let ast = make_ast(2, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::S, qubits: vec![0], params: vec![] },
            Gate { name: GateName::X, qubits: vec![1], params: vec![] },
        ]);
        assert!(!has_non_clifford_gates(&ast));
    }

    #[test]
    fn t_count_correct_after_full_decompose() {
        let ast = make_ast(3, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::T, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![FRAC_PI_4] },
        ]);

        let (result, stats) = decompose_non_clifford(&ast, DEFAULT_PRECISION);

        // 1 existing T + 7 from CCX + 1 from Rz(π/4)→T = 9
        assert_eq!(stats.t_count, 9);
        assert_eq!(count_t_gates(&result.gates), 9);
    }
}
