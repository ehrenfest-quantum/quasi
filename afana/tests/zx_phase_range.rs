//! Integration tests for ZX-IR spider phase range validation (#850).
//!
//! Phases are stored in units of π. The canonical range for a normalised
//! ZX graph is `[0, 2)`. [`ZxGraph::validate_normalized`] catches any
//! spider whose phase has drifted outside that window before QASM
//! emission.

use afana::zx_ir::{SpiderColor, ZxGraph, ZxValidationError};

#[test]
fn validate_normalized_rejects_phase_negative_one_half() {
    // AC #1: a spider with phase = -0.5 (i.e. -π/2) must fail the
    // strict normalised validation.
    let mut g = ZxGraph::new();
    g.add_spider(SpiderColor::Z, -0.5, None);

    let errs = g
        .validate_normalized()
        .expect_err("phase = -0.5 must fail strict validation");

    assert!(
        errs.iter().any(|e| matches!(
            e,
            ZxValidationError::PhaseOutOfRange { phase, .. } if *phase == -0.5
        )),
        "expected a PhaseOutOfRange error for -0.5, got {errs:?}"
    );
}

#[test]
fn validate_normalized_rejects_phase_seven() {
    // AC #2: a spider with phase = 7.0 (i.e. 7π) must fail.
    let mut g = ZxGraph::new();
    g.add_spider(SpiderColor::X, 7.0, None);

    let errs = g
        .validate_normalized()
        .expect_err("phase = 7.0 must fail strict validation");

    assert!(
        errs.iter().any(|e| matches!(
            e,
            ZxValidationError::PhaseOutOfRange { phase, .. } if *phase == 7.0
        )),
        "expected a PhaseOutOfRange error for 7.0, got {errs:?}"
    );
}

#[test]
fn normalize_phases_then_validate_succeeds() {
    // Demonstrates the intended workflow for lowering passes: produce
    // un-normalised phases freely, then `normalize_phases()` once, then
    // `validate_normalized()` as the final gate before QASM emission.
    let mut g = ZxGraph::new();
    g.add_spider(SpiderColor::Z, -0.5, None);
    g.add_spider(SpiderColor::X, 7.0, None);

    g.normalize_phases();
    g.validate_normalized()
        .expect("after normalisation all phases must be in [0, 2)");
}
