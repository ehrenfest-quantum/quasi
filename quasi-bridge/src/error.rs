// SPDX-License-Identifier: AGPL-3.0-or-later
//! Error types for quasi-bridge.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("unknown molecule: {0}")]
    UnknownMolecule(String),

    #[error("unsupported basis set: {0}")]
    UnsupportedBasis(String),

    #[error("no basis data for element Z={0}")]
    NoBasisForElement(u8),

    #[error("RHF did not converge after {iterations} iterations (dE={delta_e:.2e})")]
    RhfNotConverged { iterations: usize, delta_e: f64 },

    #[error("empty Hamiltonian: Jordan-Wigner requires at least one term")]
    EmptyHamiltonian,

    #[error("Afana compilation failed: {0}")]
    AfanaCompile(String),

    #[error("Solvayeur error: {0}")]
    Solvayeur(String),

    #[error("invalid Garm JSON: {0}")]
    GarmParse(String),

    #[error("active space too large: {n_qubits} qubits (max {max_qubits})")]
    ActiveSpaceTooLarge { n_qubits: usize, max_qubits: usize },

    #[error("{0}")]
    Other(String),
}
