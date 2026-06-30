//! Integration tests for ZX-IR single-qubit gate-sequence parity (#858).
//!
//! "Parity" of a single-qubit gate sequence in ZX-calculus terms:
//!
//! - X² = I, so two adjacent X-spiders on the same qubit-wire should
//!   have fused into a single spider with phase summed (mod 2).
//! - Z² = I (up to global phase), same story for Z-spiders.
//! - A sequence with odd vs even count of identical phase-1 spiders
//!   reduces to a different canonical form — the "parity" determines
//!   whether the wire ends up at |0⟩ or |1⟩.
//!
//! [`ZxGraph::validate_fully_fused`] is the post-optimisation gate that
//! flags any pair of edge-connected, same-color, same-qubit spiders —
//! exactly the residual single-qubit sequences this issue asks us to
//! detect.

use afana::zx_ir::{SpiderColor, ZxGraph, ZxValidationError};

/// AC #1: a brand-new validation pass detects parity issues in
/// single-qubit gate sequences. Here, two consecutive X(π) spiders on
/// qubit 0 — which compose to identity — are flagged.
#[test]
fn validation_pass_detects_x_x_residual_on_single_qubit() {
    let mut g = ZxGraph::new();
    let x1 = g.add_spider(SpiderColor::X, 1.0, Some(0));
    let x2 = g.add_spider(SpiderColor::X, 1.0, Some(0));
    g.add_edge(x1, x2);

    let errs = g
        .validate_fully_fused()
        .expect_err("two adjacent same-color spiders on one qubit must fail");

    let pair_err = errs.iter().find_map(|e| {
        if let ZxValidationError::UnfusedAdjacentSpiders { a, b, qubit } = e {
            Some((*a, *b, *qubit))
        } else {
            None
        }
    });

    let (a, b, qubit) = pair_err.expect("expected an UnfusedAdjacentSpiders error");
    assert!((a == x1 && b == x2) || (a == x2 && b == x1));
    assert_eq!(qubit, 0);
}

/// AC #1 again, with Z-spiders: ZZ chain on a single qubit-wire is also
/// the fusion candidate (Z(α)·Z(β) = Z(α+β)).
#[test]
fn validation_pass_detects_z_z_residual_on_single_qubit() {
    let mut g = ZxGraph::new();
    let z1 = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    let z2 = g.add_spider(SpiderColor::Z, 0.25, Some(0));
    g.add_edge(z1, z2);

    let errs = g.validate_fully_fused().unwrap_err();
    assert!(errs
        .iter()
        .any(|e| matches!(e, ZxValidationError::UnfusedAdjacentSpiders { qubit: 0, .. })));
}

/// AC #2: existing canonical single-qubit gate sequences pass the new
/// validation without errors. The H-gate lowering produces Z(½)-X(½)-Z(½)
/// — a canonical sequence with NO adjacent same-color same-qubit pairs.
#[test]
fn canonical_h_lowering_passes_parity_validation() {
    let mut g = ZxGraph::new();
    let inp = g.add_spider(SpiderColor::Z, 0.0, None); // boundary
    let z1 = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    let x = g.add_spider(SpiderColor::X, 0.5, Some(0));
    let z2 = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    let out = g.add_spider(SpiderColor::Z, 0.0, None); // boundary
    g.set_inputs(vec![inp]);
    g.set_outputs(vec![out]);
    g.add_edge(inp, z1);
    g.add_edge(z1, x);
    g.add_edge(x, z2);
    g.add_edge(z2, out);

    g.validate_fully_fused()
        .expect("canonical H-gate ZX form should pass parity validation");
}

/// AC #3: validation correctly distinguishes fusible vs non-fusible
/// pairs across a variety of single-qubit gate sequences. Adjacent
/// same-color spiders on DIFFERENT qubits (e.g. a CZ entangler
/// connecting Z-spiders on q0 and q1) must NOT be flagged.
#[test]
fn cz_entangler_does_not_trigger_single_qubit_parity_error() {
    let mut g = ZxGraph::new();
    // Two qubits, each starting with a single Z-spider, connected by an
    // edge representing a CZ. Same color, different qubit-wires.
    let z_q0 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
    let z_q1 = g.add_spider(SpiderColor::Z, 0.0, Some(1));
    g.add_edge(z_q0, z_q1);

    g.validate_fully_fused()
        .expect("CZ-style same-color cross-qubit edge is not a parity issue");
}

/// AC #3: a longer single-qubit chain with multiple un-fused pairs
/// produces multiple errors — one per fusible adjacency.
#[test]
fn long_residual_chain_yields_multiple_parity_errors() {
    let mut g = ZxGraph::new();
    let a = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    let b = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    let c = g.add_spider(SpiderColor::Z, 0.5, Some(0));
    g.add_edge(a, b);
    g.add_edge(b, c);

    let errs = g.validate_fully_fused().unwrap_err();
    let parity_errors: Vec<_> = errs
        .iter()
        .filter(|e| matches!(e, ZxValidationError::UnfusedAdjacentSpiders { .. }))
        .collect();
    assert_eq!(parity_errors.len(), 2, "expected 2 unfused pairs, got {parity_errors:?}");
}
