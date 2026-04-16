// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::ast::{EhrenfestAst, Gate, GateName};
use afana::lower::lower_ast_to_zx;
use afana::zx_ir::ZxGraph;

#[test]
fn test_valid_qubit_indices() {
    // Valid circuit with qubits 0 and 1
    let ast = EhrenfestAst {
        name: "valid".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let result = lower_ast_to_zx(&ast);
    assert!(result.is_ok(), "Valid qubit indices should pass validation");
}

#[test]
fn test_invalid_qubit_index() {
    // Invalid circuit: qubit 2 is out of bounds for n_qubits=2
    let ast = EhrenfestAst {
        name: "invalid".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate { name: GateName::H, qubits: vec![2], params: vec![] }, // Invalid qubit index
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let result = lower_ast_to_zx(&ast);
    assert!(result.is_err(), "Out-of-bounds qubit index should be rejected");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("out of range"), "Error should mention out of range");
}

#[test]
fn test_qubit_index_equals_n_qubits() {
    // Invalid circuit: qubit index equals n_qubits (should be < n_qubits)
    let ast = EhrenfestAst {
        name: "invalid".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate { name: GateName::H, qubits: vec![2], params: vec![] }, // n_qubits=2, so index 2 is invalid
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let result = lower_ast_to_zx(&ast);
    assert!(result.is_err(), "Qubit index equal to n_qubits should be rejected");
}

#[test]
fn test_multi_qubit_gate_invalid_index() {
    // Invalid circuit: CX gate with one valid and one invalid qubit index
    let ast = EhrenfestAst {
        name: "invalid".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate { name: GateName::Cx, qubits: vec![0, 2], params: vec![] }, // Second qubit invalid
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let result = lower_ast_to_zx(&ast);
    assert!(result.is_err(), "Multi-qubit gate with invalid index should be rejected");
}

#[test]
fn test_empty_circuit_valid() {
    // Empty circuit with n_qubits=2 is valid
    let ast = EhrenfestAst {
        name: "empty".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let result = lower_ast_to_zx(&ast);
    assert!(result.is_ok(), "Empty circuit should be valid");
}
