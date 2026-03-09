// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Unit tests for RZ gate synthesis from ZX-IR phase spiders.

use crate::ast::*;
use crate::emit::{emit_qasm, QasmVersion};

#[test]
fn test_rz_synthesis_pi_3() {
    // Create a simple AST with RZ(π/3) gate
    let ast = EhrenfestAst {
        name: "rz_synthesis_test".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![std::f64::consts::FRAC_PI_3],
            },
        ],
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    // Emit QASM 3.0
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    // Verify the output contains the expected RZ statement
    assert!(
        qasm.contains("rz(pi/3) q[0];"),
        "Expected QASM to contain 'rz(pi/3) q[0];', got: {}",
        qasm
    );
}