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

    #[error("unbound parameter `{param}` in variational gate `{gate}` (declared params: {declared:?})")]
    UnboundParameter {
        param: String,
        gate: String,
        declared: Vec<String>,
    },

    #[error("missing binding for parameter `{param}` in variational gate `{gate}`")]
    MissingBinding {
        param: String,
        gate: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_decode_renders_message() {
        let err = CborError::Decode("truncated input".to_string());
        assert_eq!(err.to_string(), "CBOR decode: truncated input");
    }

    #[test]
    fn cbor_schema_renders_message() {
        let err = CborError::Schema("expected array".to_string());
        assert_eq!(err.to_string(), "schema violation: expected array");
    }

    #[test]
    fn cbor_io_renders_message_and_supports_from_conversion() {
        let io_err = std::io::Error::other("read failed");
        let err: CborError = io_err.into();
        assert_eq!(err.to_string(), "read failed");
        match err {
            CborError::Io(_) => {}
            other => panic!("expected CborError::Io, got {other:?}"),
        }
    }

    #[test]
    fn cbor_error_debug_names_its_variant() {
        let err = CborError::Decode("test".to_string());
        assert_eq!(format!("{err:?}"), r#"Decode("test")"#);
    }

    #[test]
    fn unsupported_gate_renders_name() {
        let err = EmitError::UnsupportedGate("foo".to_string());
        assert_eq!(err.to_string(), "unsupported gate: foo");
    }

    #[test]
    fn qubit_out_of_range_renders_index_and_total() {
        let err = EmitError::QubitOutOfRange {
            index: 5,
            n_qubits: 3,
        };
        assert_eq!(err.to_string(), "qubit index 5 out of range (n_qubits=3)");
    }

    #[test]
    fn unbound_parameter_renders_with_declared_debug() {
        let err = EmitError::UnboundParameter {
            param: "theta".to_string(),
            gate: "rx".to_string(),
            declared: vec!["phi".to_string()],
        };
        assert_eq!(
            err.to_string(),
            "unbound parameter `theta` in variational gate `rx` (declared params: [\"phi\"])"
        );
    }

    #[test]
    fn missing_binding_renders_parameter_and_gate() {
        let err = EmitError::MissingBinding {
            param: "theta".to_string(),
            gate: "rx".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "missing binding for parameter `theta` in variational gate `rx`"
        );
    }

    #[test]
    fn emit_error_debug_names_its_variant() {
        let err = EmitError::UnsupportedGate("H".to_string());
        assert_eq!(format!("{err:?}"), r#"UnsupportedGate("H")"#);
    }
}
