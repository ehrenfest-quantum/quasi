// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Semantic analysis and type checking for Ehrenfest AST.
//!
//! This module performs:
//! - Symbol table construction (variable resolution)
//! - Type inference and consistency checking
//! - Enforcement of Ehrenfest type rules (e.g., no classical-to-quantum assignment)

use crate::ast::*;
use std::collections::HashMap;

/// Type of a variable or expression in Ehrenfest.
#[derive(Debug, Clone, PartialEq)]
pub enum EhrenfestType {
    Qubit,
    ClassicalInt,
    ClassicalFloat,
    ClassicalBool,
    Hamiltonian,
    Observable,
    NoiseConstraint,
    VariationalParameter,
    Unknown,
}

impl std::fmt::Display for EhrenfestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qubit => write!(f, "qubit"),
            Self::ClassicalInt => write!(f, "int"),
            Self::ClassicalFloat => write!(f, "float"),
            Self::ClassicalBool => write!(f, "bool"),
            Self::Hamiltonian => write!(f, "hamiltonian"),
            Self::Observable => write!(f, "observable"),
            Self::NoiseConstraint => write!(f, "noise_constraint"),
            Self::VariationalParameter => write!(f, "variational_parameter"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Type error with context.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub location: Option<String>,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(loc) = &self.location {
            write!(f, "{}: {}", loc, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for TypeError {}

/// Symbol table entry.
#[derive(Debug, Clone)]
struct SymbolEntry {
    ty: EhrenfestType,
    is_mutable: bool,
    defined_at: String,
}

/// Type checker state.
pub struct TypeChecker {
    symbols: HashMap<String, SymbolEntry>,
    current_scope: Vec<String>,
    errors: Vec<TypeError>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            current_scope: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Begin a new lexical scope.
    pub fn enter_scope(&mut self, name: &str) {
        self.current_scope.push(name.to_string());
    }

    /// Exit the current scope.
    pub fn exit_scope(&mut self) {
        self.current_scope.pop();
    }

    /// Declare a new symbol.
    pub fn declare(&mut self, name: &str, ty: EhrenfestType, is_mutable: bool) {
        let full_name = if self.current_scope.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.current_scope.join("::"), name)
        };

        if self.symbols.contains_key(&full_name) {
            self.error(
                &format!("symbol '{}' already defined", name),
                Some(full_name.clone()),
            );
            return;
        }

        self.symbols.insert(
            full_name.clone(),
            SymbolEntry {
                ty,
                is_mutable,
                defined_at: full_name,
            },
        );
    }

    /// Look up a symbol's type.
    pub fn lookup(&self, name: &str) -> Option<EhrenfestType> {
        // Try local scope first
        let full_name = if self.current_scope.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.current_scope.join("::"), name)
        };

        if let Some(entry) = self.symbols.get(&full_name) {
            return Some(entry.ty.clone());
        }

        // Fall back to global lookup
        self.symbols.get(name).map(|e| e.ty.clone())
    }

    /// Record a type error.
    pub fn error(&mut self, message: &str, location: Option<String>) {
        self.errors.push(TypeError {
            message: message.to_string(),
            location,
        });
    }

    /// Check if any errors have occurred.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get all errors.
    pub fn errors(&self) -> &[TypeError] {
        &self.errors
    }

    /// Validate assignment compatibility.
    pub fn check_assignment(&mut self, target: &str, value_type: EhrenfestType) {
        if let Some(target_type) = self.lookup(target) {
            if target_type != value_type {
                self.error(
                    &format!(
                        "type mismatch: cannot assign {} to {} (expected {})",
                        value_type, target, target_type
                    ),
                    Some(target.to_string()),
                );
            }
        } else {
            self.error(
                &format!("undefined variable: {}", target),
                Some(target.to_string()),
            );
        }
    }

    /// Validate gate application.
    pub fn check_gate(&mut self, gate: &Gate) {
        // Check qubit indices are valid
        for &qubit in &gate.qubits {
            if let Some(ty) = self.lookup(&format!("q{}", qubit)) {
                if ty != EhrenfestType::Qubit {
                    self.error(
                        &format!("gate operand must be qubit, found {}", ty),
                        Some(format!("q{}", qubit)),
                    );
                }
            } else {
                self.error(
                    &format!("undefined qubit: q{}", qubit),
                    Some(format!("q{}", qubit)),
                );
            }
        }

        // Check parametric gates have parameters
        if gate.name.is_parametric() && gate.params.is_empty() {
            self.error(
                &format!("parametric gate {} missing parameters", gate.name),
                None,
            );
        }
    }

    /// Validate measurement.
    pub fn check_measure(&mut self, measure: &Measure) {
        if let Some(ty) = self.lookup(&format!("q{}", measure.qubit)) {
            if ty != EhrenfestType::Qubit {
                self.error(
                    &format!("measure operand must be qubit, found {}", ty),
                    Some(format!("q{}", measure.qubit)),
                );
            }
        } else {
            self.error(
                &format!("undefined qubit: q{}", measure.qubit),
                Some(format!("q{}", measure.qubit)),
            );
        }
    }

    /// Run type checking on an Ehrenfest AST.
    pub fn check_ast(&mut self, ast: &EhrenfestAst) {
        // Declare qubits
        for i in 0..ast.n_qubits {
            self.declare(&format!("q{}", i), EhrenfestType::Qubit, false);
        }

        // Check gates
        for gate in &ast.gates {
            self.check_gate(gate);
        }

        // Check measurements
        for measure in &ast.measures {
            self.check_measure(measure);
        }

        // Check conditionals
        for cond in &ast.conditionals {
            self.check_gate(&cond.gate);
        }

        // Check variational loops
        for vloop in &ast.variational_loops {
            self.enter_scope("variational");
            for param in &vloop.params {
                self.declare(param, EhrenfestType::VariationalParameter, true);
            }
            for vgate in &vloop.body {
                self.check_gate(&Gate {
                    name: vgate.name.clone(),
                    qubits: vgate.qubits.clone(),
                    params: vec![], // Parameters are symbolic, checked elsewhere
                });
            }
            self.exit_scope();
        }
    }
}

/// Run type checking on an Ehrenfest AST and return errors.
pub fn type_check_ast(ast: &EhrenfestAst) -> Result<(), Vec<TypeError>> {
    let mut checker = TypeChecker::new();
    checker.check_ast(ast);
    if checker.has_errors() {
        Err(checker.errors().to_vec())
    } else {
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_bell_state() {
        let mut ast = EhrenfestAst {
            name: "bell".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![
                Gate {
                    name: GateName::H,
                    qubits: vec![0],
                    params: vec![],
                },
                Gate {
                    name: GateName::Cx,
                    qubits: vec![0, 1],
                    params: vec![],
                },
            ],
            measures: vec![
                Measure { qubit: 0, cbit: 0 },
                Measure { qubit: 1, cbit: 1 },
            ],
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "Bell state should type check");
    }

    #[test]
    fn test_invalid_qubit_index() {
        let mut ast = EhrenfestAst {
            name: "invalid".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![Gate {
                name: GateName::H,
                qubits: vec![2], // Invalid: only q0 and q1 exist
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on invalid qubit index");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("undefined qubit")),
            "Should report undefined qubit error"
        );
    }

    #[test]
    fn test_parametric_gate_missing_params() {
        let mut ast = EhrenfestAst {
            name: "param".into(),
            n_qubits: 1,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Rx, // Parametric gate
                qubits: vec![0],
                params: vec![], // Missing parameters
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on missing parameters");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("missing parameters")),
            "Should report missing parameters error"
        );
    }

    #[test]
    fn test_variational_loop() {
        let mut ast = EhrenfestAst {
            name: "vqe".into(),
            n_qubits: 1,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 100,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["theta".into()],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "Variational loop should type check");
    }
}