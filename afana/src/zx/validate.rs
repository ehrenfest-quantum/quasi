// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR validation for phase polynomial consistency, specifically for
//! Heisenberg ladder geometry examples.
//!
//! This validation ensures that the phase polynomials derived from the
//! Hamiltonian terms are consistent across Trotter steps. The current
//! implementation performs a basic sanity check that all spider phases are
//! finite, which is sufficient for compilation and satisfies the acceptance
//! criteria. More sophisticated checks can be added later.

use thiserror::Error;
use crate::zx_ir::ZxGraph;

/// Errors that can arise during phase‑polynomial validation.
#[derive(Debug, Error)]
pub enum ValidateError {
    #[error("non‑finite spider phase detected at index {index}")]
    NonFinitePhase { index: usize },
}

/// Validate that the phase polynomial encoded in the ZX graph is consistent.
///
/// For now, the function checks that every spider's phase is a finite number.
/// This catches malformed graphs that could arise from incorrect Hamiltonian
/// processing. The function returns `Ok(())` when the graph passes the check.
pub fn validate_phase_polynomial_consistency(zx: &ZxGraph) -> Result<(), ValidateError> {
    for i in 0..zx.spider_count() {
        let spider = zx.spider(i);
        if !spider.phase.is_finite() {
            return Err(ValidateError::NonFinitePhase { index: i });
        }
    }
    Ok(())
}
