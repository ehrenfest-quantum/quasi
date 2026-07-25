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
}

/// Type checker state.
pub struct TypeChecker {
    symbols: HashMap<String, SymbolEntry>,
    current_scope: Vec<String>,
    errors: Vec<TypeError>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
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
    pub fn declare(&mut self, name: &str, ty: EhrenfestType) {
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

        self.symbols.insert(full_name, SymbolEntry { ty });
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

    /// Validate gate connectivity: multi-qubit gates must reference distinct qubits,
    /// and the qubit count must match the gate's arity.
    pub fn validate_connectivity(&mut self, gate: &Gate) {
        let arity = gate.name.arity();

        // Check arity match
        if gate.qubits.len() != arity {
            self.error(
                &format!(
                    "gate {} expects {} qubit(s), got {}",
                    gate.name,
                    arity,
                    gate.qubits.len()
                ),
                Some(format!("gate {}", gate.name)),
            );
            return;
        }

        // For multi-qubit gates, check all qubits are distinct
        if arity >= 2 {
            let mut seen = std::collections::HashSet::new();
            for &qubit in &gate.qubits {
                if !seen.insert(qubit) {
                    self.error(
                        &format!(
                            "gate {} references qubit q[{}] more than once — operands must be distinct",
                            gate.name, qubit
                        ),
                        Some(format!("gate {}", gate.name)),
                    );
                }
            }
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

    /// Validate connectivity for a variational gate: multi-qubit gates must
    /// reference distinct qubits.
    pub fn validate_variational_connectivity(&mut self, vgate: &VariationalGate) {
        let arity = vgate.name.arity();
        if arity >= 2 && vgate.qubits.len() == arity {
            let mut seen = std::collections::HashSet::new();
            for &qubit in &vgate.qubits {
                if !seen.insert(qubit) {
                    self.error(
                        &format!(
                            "gate {} references qubit q[{}] more than once — operands must be distinct",
                            vgate.name, qubit
                        ),
                        Some(format!("variational gate {}", vgate.name)),
                    );
                }
            }
        }
    }

    /// Validate a variational gate within a loop.
    ///
    /// Checks that:
    /// - All `param_refs` resolve to declared variational parameters
    /// - Qubit indices are within the declared `n_qubits` range
    /// - Qubit arity matches the gate definition
    /// - Parametric gates (Rx, Ry, Rz) have at least one `param_ref`
    pub fn check_variational_gate(
        &mut self,
        vgate: &VariationalGate,
        declared_params: &std::collections::HashSet<&str>,
        n_qubits: usize,
    ) {
        // Check qubit indices are within range
        for &qubit in &vgate.qubits {
            if qubit >= n_qubits {
                self.error(
                    &format!(
                        "qubit index {} out of range (n_qubits={})",
                        qubit, n_qubits
                    ),
                    Some(format!("variational gate {}", vgate.name)),
                );
            }
        }

        // Check qubit arity matches gate definition
        let expected_arity = vgate.name.arity();
        if vgate.qubits.len() != expected_arity {
            self.error(
                &format!(
                    "gate {} expects {} qubit(s), got {}",
                    vgate.name,
                    expected_arity,
                    vgate.qubits.len()
                ),
                Some(format!("variational gate {}", vgate.name)),
            );
        }

        // Check all param_refs resolve to declared parameters
        for pref in &vgate.param_refs {
            if !declared_params.contains(pref.as_str()) {
                self.error(
                    &format!(
                        "unbound parameter '{}' in variational gate {}",
                        pref, vgate.name
                    ),
                    Some(format!("variational gate {}", vgate.name)),
                );
            }
        }

        // Parametric gates must have param_refs in variational context
        if vgate.name.is_parametric() && vgate.param_refs.is_empty() {
            self.error(
                &format!(
                    "parametric gate {} requires param_refs in variational context",
                    vgate.name
                ),
                Some(format!("variational gate {}", vgate.name)),
            );
        }
    }

    /// Validate a variational loop.
    ///
    /// Checks structural well-formedness:
    /// - No duplicate parameter names within the loop
    /// - Loop body must be non-empty
    /// - `max_iter` must be greater than zero
    /// - All gates in the body pass variational gate checks
    pub fn check_variational_loop(&mut self, vloop: &VariationalLoop, n_qubits: usize) {
        // Check for duplicate parameter names
        let mut seen_params = std::collections::HashSet::new();
        for param in &vloop.params {
            if !seen_params.insert(param.as_str()) {
                self.error(
                    &format!("duplicate parameter '{}' in variational loop", param),
                    Some("variational loop".to_string()),
                );
            }
        }

        // Check loop body is non-empty
        if vloop.body.is_empty() {
            self.error(
                "variational loop body must not be empty",
                Some("variational loop".to_string()),
            );
        }

        // Check max_iter > 0
        if vloop.max_iter == 0 {
            self.error(
                "variational loop max_iter must be greater than zero",
                Some("variational loop".to_string()),
            );
        }

        let declared_params: std::collections::HashSet<&str> =
            vloop.params.iter().map(|s| s.as_str()).collect();

        self.enter_scope("variational");
        for param in &vloop.params {
            self.declare(param, EhrenfestType::VariationalParameter);
        }
        for vgate in &vloop.body {
            self.check_variational_gate(vgate, &declared_params, n_qubits);
            self.validate_variational_connectivity(vgate);
        }
        self.exit_scope();
    }

    /// Run type checking on an Ehrenfest AST.
    pub fn check_ast(&mut self, ast: &EhrenfestAst) {
        // Declare qubits
        for i in 0..ast.n_qubits {
            self.declare(&format!("q{}", i), EhrenfestType::Qubit);
        }

        // Check gates
        for gate in &ast.gates {
            self.check_gate(gate);
            self.validate_connectivity(gate);
        }

        // Check measurements
        for measure in &ast.measures {
            self.check_measure(measure);
        }

        // Check conditionals
        for cond in &ast.conditionals {
            self.check_gate(&cond.gate);
            self.validate_connectivity(&cond.gate);
        }

        // Check variational loops
        for vloop in &ast.variational_loops {
            self.check_variational_loop(vloop, ast.n_qubits);
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
        let ast = EhrenfestAst {
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
        let ast = EhrenfestAst {
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
        let ast = EhrenfestAst {
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
        let ast = EhrenfestAst {
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

    #[test]
    fn test_variational_loop_multi_param() {
        let ast = EhrenfestAst {
            name: "vqe_multi".into(),
            n_qubits: 2,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into(), "phi".into()],
                max_iter: 100,
                body: vec![
                    VariationalGate {
                        name: GateName::Ry,
                        qubits: vec![0],
                        param_refs: vec!["theta".into()],
                    },
                    VariationalGate {
                        name: GateName::Rz,
                        qubits: vec![1],
                        param_refs: vec!["phi".into()],
                    },
                    VariationalGate {
                        name: GateName::Cx,
                        qubits: vec![0, 1],
                        param_refs: vec![],
                    },
                ],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "Multi-param variational loop should type check");
    }

    #[test]
    fn test_variational_unbound_param() {
        let ast = EhrenfestAst {
            name: "bad_vqe".into(),
            n_qubits: 1,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["gamma".into()],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on unbound param_ref");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("unbound parameter 'gamma'")),
            "Should report unbound parameter error"
        );
    }

    #[test]
    fn test_variational_qubit_out_of_range() {
        let ast = EhrenfestAst {
            name: "bad_qubit".into(),
            n_qubits: 2,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![5], // out of range
                    param_refs: vec!["theta".into()],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on out-of-range qubit");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("qubit index 5 out of range")),
            "Should report qubit out of range error"
        );
    }

    #[test]
    fn test_variational_parametric_gate_missing_param_refs() {
        let ast = EhrenfestAst {
            name: "no_params".into(),
            n_qubits: 1,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Rx,
                    qubits: vec![0],
                    param_refs: vec![], // Rx needs param_refs in variational context
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail when parametric gate lacks param_refs");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("requires param_refs")),
            "Should report missing param_refs for parametric gate"
        );
    }

    #[test]
    fn test_variational_loop_duplicate_params() {
        let ast = EhrenfestAst {
            name: "dup_params".into(),
            n_qubits: 2,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into(), "phi".into(), "theta".into()],
                max_iter: 100,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["theta".into()],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on duplicate parameter names");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("duplicate parameter 'theta'")),
            "Should report duplicate parameter error"
        );
    }

    #[test]
    fn test_variational_loop_empty_body() {
        let ast = EhrenfestAst {
            name: "empty_body".into(),
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
                body: vec![],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on empty loop body");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("body must not be empty")),
            "Should report empty body error"
        );
    }

    #[test]
    fn test_variational_loop_zero_max_iter() {
        let ast = EhrenfestAst {
            name: "zero_iter".into(),
            n_qubits: 1,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 0,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["theta".into()],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on zero max_iter");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("max_iter must be greater than zero")),
            "Should report zero max_iter error"
        );
    }

    #[test]
    fn test_variational_loop_valid_nested_structure() {
        // Multiple well-formed variational loops in the same program
        let ast = EhrenfestAst {
            name: "nested_valid".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![Gate {
                name: GateName::H,
                qubits: vec![0],
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![
                VariationalLoop {
                    params: vec!["alpha".into(), "beta".into()],
                    max_iter: 200,
                    body: vec![
                        VariationalGate {
                            name: GateName::Rx,
                            qubits: vec![0],
                            param_refs: vec!["alpha".into()],
                        },
                        VariationalGate {
                            name: GateName::Ry,
                            qubits: vec![1],
                            param_refs: vec!["beta".into()],
                        },
                        VariationalGate {
                            name: GateName::Cx,
                            qubits: vec![0, 1],
                            param_refs: vec![],
                        },
                    ],
                },
                VariationalLoop {
                    params: vec!["gamma".into()],
                    max_iter: 50,
                    body: vec![VariationalGate {
                        name: GateName::Rz,
                        qubits: vec![2],
                        param_refs: vec!["gamma".into()],
                    }],
                },
            ],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "Valid nested variational structure should type check");
    }

    #[test]
    fn test_variational_gate_arity_mismatch() {
        let ast = EhrenfestAst {
            name: "bad_arity".into(),
            n_qubits: 3,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Cx, // expects 2 qubits
                    qubits: vec![0],    // only 1
                    param_refs: vec![],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Should fail on qubit arity mismatch");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("expects 2 qubit(s), got 1")),
            "Should report arity mismatch"
        );
    }

    // ── Connectivity validation tests ─────────────────────────────────────

    #[test]
    fn test_cx_duplicate_qubit_rejected() {
        let ast = EhrenfestAst {
            name: "dup_cx".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Cx,
                qubits: vec![0, 0], // same qubit twice
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "CX with duplicate qubits should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("more than once")),
            "Should report duplicate qubit: {:?}",
            errors
        );
    }

    #[test]
    fn test_cz_duplicate_qubit_rejected() {
        let ast = EhrenfestAst {
            name: "dup_cz".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Cz,
                qubits: vec![1, 1],
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "CZ with duplicate qubits should fail");
    }

    #[test]
    fn test_swap_duplicate_qubit_rejected() {
        let ast = EhrenfestAst {
            name: "dup_swap".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Swap,
                qubits: vec![0, 0],
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "SWAP with duplicate qubits should fail");
    }

    #[test]
    fn test_ccx_duplicate_qubit_rejected() {
        let ast = EhrenfestAst {
            name: "dup_ccx".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Ccx,
                qubits: vec![0, 1, 0], // q0 appears twice
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "CCX with duplicate qubits should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("q[0]") && e.message.contains("more than once")),
            "Should report duplicate qubit q[0]: {:?}",
            errors
        );
    }

    #[test]
    fn test_ccx_all_distinct_qubits_accepted() {
        let ast = EhrenfestAst {
            name: "good_ccx".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Ccx,
                qubits: vec![0, 1, 2],
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "CCX with 3 distinct qubits should pass");
    }

    #[test]
    fn test_cx_arity_mismatch_rejected() {
        let ast = EhrenfestAst {
            name: "cx_one_qubit".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![Gate {
                name: GateName::Cx,
                qubits: vec![0], // CX needs 2
                params: vec![],
            }],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "CX with 1 qubit should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("expects 2 qubit(s), got 1")),
            "Should report arity mismatch: {:?}",
            errors
        );
    }

    #[test]
    fn test_single_qubit_gate_no_false_positive() {
        // Single-qubit gates should not trigger connectivity errors
        let ast = EhrenfestAst {
            name: "single".into(),
            n_qubits: 1,
            prepare: None,
            gates: vec![
                Gate { name: GateName::H, qubits: vec![0], params: vec![] },
                Gate { name: GateName::X, qubits: vec![0], params: vec![] },
                Gate { name: GateName::Rz, qubits: vec![0], params: vec![1.57] },
            ],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };

        let result = type_check_ast(&ast);
        assert!(result.is_ok(), "Single-qubit gates should pass connectivity check");
    }

    #[test]
    fn test_variational_cx_duplicate_qubit_rejected() {
        let ast = EhrenfestAst {
            name: "var_dup".into(),
            n_qubits: 2,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Cx,
                    qubits: vec![0, 0], // duplicate
                    param_refs: vec![],
                }],
            }],
        };

        let result = type_check_ast(&ast);
        assert!(result.is_err(), "Variational CX with duplicate qubits should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains("more than once")),
            "Should report duplicate qubit in variational gate: {:?}",
            errors
        );
    }

    // ── Display rendering tests ───────────────────────────────────────────

    #[test]
    fn qubit_type_renders_name() {
        assert_eq!(EhrenfestType::Qubit.to_string(), "qubit");
    }

    #[test]
    fn classical_int_renders_name() {
        assert_eq!(EhrenfestType::ClassicalInt.to_string(), "int");
    }

    #[test]
    fn classical_float_renders_name() {
        assert_eq!(EhrenfestType::ClassicalFloat.to_string(), "float");
    }

    #[test]
    fn classical_bool_renders_name() {
        assert_eq!(EhrenfestType::ClassicalBool.to_string(), "bool");
    }

    #[test]
    fn hamiltonian_type_renders_name() {
        assert_eq!(EhrenfestType::Hamiltonian.to_string(), "hamiltonian");
    }

    #[test]
    fn observable_type_renders_name() {
        assert_eq!(EhrenfestType::Observable.to_string(), "observable");
    }

    #[test]
    fn noise_constraint_type_renders_name() {
        assert_eq!(
            EhrenfestType::NoiseConstraint.to_string(),
            "noise_constraint"
        );
    }

    #[test]
    fn variational_parameter_type_renders_name() {
        assert_eq!(
            EhrenfestType::VariationalParameter.to_string(),
            "variational_parameter"
        );
    }

    #[test]
    fn unknown_type_renders_name() {
        assert_eq!(EhrenfestType::Unknown.to_string(), "unknown");
    }
}
