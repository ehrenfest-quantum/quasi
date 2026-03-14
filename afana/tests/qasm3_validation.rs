// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::ast::{EhrenfestAst, Gate, GateName};
use afana::emit::{emit_qasm, QasmVersion};

#[test]
fn test_h_gate_qasm3_syntax_validation() {
    // Build minimal Ehrenfest AST with H-gate
    let ast = EhrenfestAst {
        name: "h_gate_test".to_string(),
        n_qubits: 1,
        prepare: None,
        gates: vec![Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        }],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    // Emit QASM3 syntax
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM emission failed");

    // Validate H-gate syntax
    assert!(
        qasm.contains("h q[0];"),
        "QASM3 output does not contain expected H-gate syntax: {}",
        qasm
    );
}
