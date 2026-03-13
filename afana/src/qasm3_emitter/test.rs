// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Tests for QASM3 emission of CZ gates.

use afana::ast::*;
use afana::emit::{emit_qasm, QasmVersion};

#[test]
fn validate_cz_gate() {
    let ast = EhrenfestAst {
        name: "cz_test".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::Cz,
                qubits: vec![0, 1],
                params: vec![]
            }
        ],
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new()
    };
    
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    assert!(qasm.contains("cz qubit[0], qubit[1];"), "QASM3 output must contain 'cz qubit[0], qubit[1];'");
}