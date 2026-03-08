// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Parameterized gate synthesis from ZX-IR nodes.
//!
//! This module handles the translation of ZX-calculus optimizations into
//! QASM3-compatible parameterized gates (e.g., U3, Rx, Ry, Rz).

use crate::ast::*;
use crate::error::EmitError;

/// A parameterized gate in the ZX-IR.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterizedGate {
    pub name: GateName,
    pub qubits: Vec<usize>,
    pub parameters: Vec<Parameter>,
}

/// A parameter that can be bound to a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    /// A concrete floating-point value.
    Constant(f64),
    /// A symbolic parameter reference.
    Symbolic(String),
}

impl std::fmt::Display for Parameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parameter::Constant(val) => write!(f, "{}", val),
            Parameter::Symbolic(name) => write!(f, "{}", name),
        }
    }
}

/// Synthesize parameterized gates from ZX-IR nodes.
///
/// This function takes a ZX graph (represented as a list of gates)
/// and produces a sequence of parameterized gates suitable for QASM3 output.
pub fn synthesize_parameterized_gates(
    gates: &[Gate],
    variational_params: &[String],
) -> Vec<ParameterizedGate> {
    let mut result = Vec::new();

    for gate in gates {
        if !gate.name.is_parametric() && gate.params.is_empty() {
            // Non-parametric gate with no parameters
            result.push(ParameterizedGate {
                name: gate.name.clone(),
                qubits: gate.qubits.clone(),
                parameters: vec![],
            });
        } else {
            // Handle parametric gates or gates with parameters
            let parameters = gate
                .params
                .iter()
                .map(|&p| Parameter::Constant(p))
                .collect();

            result.push(ParameterizedGate {
                name: gate.name.clone(),
                qubits: gate.qubits.clone(),
                parameters,
            });
        }
    }

    result
}

/// Emit QASM3 statements for parameterized gates.
pub fn emit_parameterized_qasm(
    gates: &[ParameterizedGate],
    version: QasmVersion,
) -> Result<String, EmitError> {
    let mut lines = Vec::new();

    for gate in gates {
        let qubit_args: String = gate
            .qubits
            .iter()
            .map(|idx| format!("q[{}]", idx))
            .collect::<Vec<_>>()
            .join(", ");

        if gate.parameters.is_empty() {
            lines.push(format!("{} {};", gate.name.as_str(), qubit_args));
        } else {
            let param_str = gate
                .parameters
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "{}({}) {};",
                gate.name.as_str(),
                param_str,
                qubit_args
            ));
        }
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_parametric_gate() {
        let gates = vec![
            Gate {
                name: GateName::Rx,
                qubits: vec![0],
                params: vec![1.57],
            },
            Gate {
                name: GateName::H,
                qubits: vec![1],
                params: vec![],
            },
        ];

        let param_gates = synthesize_parameterized_gates(&gates, &[]);
        assert_eq!(param_gates.len(), 2);
        assert_eq!(param_gates[0].name, GateName::Rx);
        assert_eq!(param_gates[0].parameters, vec![Parameter::Constant(1.57)]);
        assert_eq!(param_gates[1].name, GateName::H);
        assert!(param_gates[1].parameters.is_empty());
    }

    #[test]
    fn test_emit_parameterized_qasm() {
        let gates = vec![ParameterizedGate {
            name: GateName::Rx,
            qubits: vec![0],
            parameters: vec![Parameter::Constant(1.57)],
        }];

        let qasm = emit_parameterized_qasm(&gates, QasmVersion::V3).unwrap();
        assert!(qasm.contains("rx(1.57) q[0];"));
    }
}
