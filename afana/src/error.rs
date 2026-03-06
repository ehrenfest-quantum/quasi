// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Compiler error types.

use thiserror::Error;

/// Errors raised during CBOR deserialization of Ehrenfest binary programs.
#[derive(Debug, Error)]
pub enum CborError {
    #[error("CBOR decode: {0}")]
    Decode(String),

    #[error("schema violation: {0}")]
    Schema(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Errors raised during QASM emission.
#[derive(Debug, Error)]
pub enum EmitError {
    #[error("unsupported gate: {0}")]
    UnsupportedGate(String),

    #[error("qubit index {index} out of range (n_qubits={n_qubits})")]
    QubitOutOfRange { index: usize, n_qubits: usize },
}
