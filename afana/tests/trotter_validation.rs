// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::zx_ir::{ZxGraph, ZxValidationError};

#[test]
fn valid_trotter_steps_accepted() {
    let graph = ZxGraph::new();
    let result = graph.validate_trotter_steps(
        100,    // steps
        100.0,  // t1_us
        50.0,   // t2_us
        1.0,    // hamiltonian_norm
        1.0,    // total_time_us
    );
    assert!(result.is_ok(), "Valid step count should be accepted");
}

#[test]
fn excessive_trotter_steps_rejected() {
    let graph = ZxGraph::new();
    let result = graph.validate_trotter_steps(
        1000,   // steps
        100.0,  // t1_us
        50.0,   // t2_us
        1.0,    // hamiltonian_norm
        1.0,    // total_time_us
    );
    
    assert!(matches!(
        result,
        Err(ZxValidationError::TrotterStepConstraint { steps: 1000, max_steps: _ })
    ), "Excessive step count should be rejected");
}

#[test]
fn step_validation_respects_t1_t2() {
    let graph = ZxGraph::new();
    let result = graph.validate_trotter_steps(
        200,    // steps
        100.0,  // t1_us
        50.0,   // t2_us (more restrictive)
        1.0,    // hamiltonian_norm
        1.0,    // total_time_us
    );
    
    // Should be limited by T2 (50.0 / 0.005 = 100 steps max)
    assert!(matches!(
        result,
        Err(ZxValidationError::TrotterStepConstraint { steps: 200, max_steps: 100 })
    ));
}

#[test]
fn step_validation_respects_hamiltonian_norm() {
    let graph = ZxGraph::new();
    let result = graph.validate_trotter_steps(
        100,    // steps
        1000.0, // t1_us
        500.0,  // t2_us
        10.0,   // hamiltonian_norm (more restrictive)
        1.0,    // total_time_us
    );
    
    // Should be limited by Hamiltonian norm (1/(10.0 * 0.01) = 10 steps max)
    assert!(matches!(
        result,
        Err(ZxValidationError::TrotterStepConstraint { steps: 100, max_steps: 10 })
    ));
}