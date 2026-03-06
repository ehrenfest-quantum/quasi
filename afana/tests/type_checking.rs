// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Type checking tests for Ehrenfest AST.

use afana::ast::*;
use afana::type_check::*;

#[test]
fn test_valid_qubit_operations() {
    let ast = EhrenfestAst {
        name: "valid_ops".into(),
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
    assert!(result.is_ok(), "Valid qubit operations should pass type checking");
}

#[test]
fn test_invalid_qubit_index() {
    let ast = EhrenfestAst {
        name: "invalid_qubit".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![Gate {
            name: GateName::X,
            qubits: vec![1], // Invalid: only q0 exists
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
fn test_parametric_gate_without_params() {
    let ast = EhrenfestAst {
        name: "missing_params".into(),
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
fn test_measure_invalid_qubit() {
    let ast = EhrenfestAst {
        name: "invalid_measure".into(),
        n_qubits: 1,
        prepare: None,
        gates: Vec::new(),
        measures: vec![Measure {
            qubit: 1, // Invalid: only q0 exists
            cbit: 0,
        }],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should fail on invalid measurement qubit");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("undefined qubit")),
        "Should report undefined qubit error"
    );
}

#[test]
fn test_variational_loop_valid() {
    let ast = EhrenfestAst {
        name: "vqe_valid".into(),
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
    assert!(result.is_ok(), "Valid variational loop should pass type checking");
}

#[test]
fn test_variational_loop_invalid_qubit() {
    let ast = EhrenfestAst {
        name: "vqe_invalid".into(),
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
                qubits: vec![1], // Invalid: only q0 exists
                param_refs: vec!["theta".into()],
            }],
        }],
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should fail on invalid qubit in variational loop");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("undefined qubit")),
        "Should report undefined qubit error"
    );
}

#[test]
fn test_conditional_gate_valid() {
    let ast = EhrenfestAst {
        name: "conditional_valid".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        }],
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: vec![ConditionalGate {
            cbit: 0,
            cbit_value: 1,
            gate: Gate {
                name: GateName::X,
                qubits: vec![1],
                params: vec![],
            },
        }],
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Valid conditional gate should pass type checking");
}

#[test]
fn test_conditional_gate_invalid_qubit() {
    let ast = EhrenfestAst {
        name: "conditional_invalid".into(),
        n_qubits: 1,
        prepare: None,
        gates: Vec::new(),
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: vec![ConditionalGate {
            cbit: 0,
            cbit_value: 1,
            gate: Gate {
                name: GateName::X,
                qubits: vec![1], // Invalid: only q0 exists
                params: vec![],
            },
        }],
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should fail on invalid qubit in conditional");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("undefined qubit")),
        "Should report undefined qubit error"
    );
}