// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::ast::*;
use afana::emit::{emit_qasm, QasmVersion};
use afana::synthesis::*;
use afana::trotter::*;
use afana::type_check::*;

#[test]
fn test_y_gate_synthesis() {
    // Create a simple AST with a Y gate
    let ast = EhrenfestAst {
        name: "y_gate_test".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![Gate {
            name: GateName::Y,
            qubits: vec![0],
            params: vec![],
        }],
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    // Type check the AST
    assert!(type_check_ast(&ast).is_ok());

    // Emit QASM 3.0 and verify it contains 'y q[0]'
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    assert!(qasm.contains("y q[0];"));
}
