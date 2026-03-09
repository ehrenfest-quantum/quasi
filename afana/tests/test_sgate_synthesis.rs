// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::ast::GateName;
use afana::synthesis::synthesize_s_gate;
use std::f64::consts::{FRAC_PI_2, PI};

#[test]
fn test_s_gate_synthesis() {
    // Test S gate (π/2 phase)
    let gate = synthesize_s_gate(FRAC_PI_2).unwrap();
    assert_eq!(gate.name, GateName::S);
    assert!(gate.params.is_empty());

    // Test Sdg gate (-π/2 phase)
    let gate = synthesize_s_gate(-FRAC_PI_2).unwrap();
    assert_eq!(gate.name, GateName::Sdg);
    assert!(gate.params.is_empty());

    // Test unsupported phases
    assert!(synthesize_s_gate(0.0).is_none());
    assert!(synthesize_s_gate(PI).is_none());
    assert!(synthesize_s_gate(FRAC_PI_2 + 0.1).is_none());
}

#[test]
fn test_s_gate_qasm_emission() {
    use afana::ast::{EhrenfestAst, Gate};
    use afana::emit::{emit_qasm, QasmVersion};

    // Create a simple AST with S and Sdg gates
    let ast = EhrenfestAst {
        name: "s_gate_test".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![Gate {
            name: GateName::S,
            qubits: vec![0],
            params: vec![],
        }],
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    // Emit QASM 3.0 and verify S gate is present
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    assert!(qasm.contains("s q[0];"));
}
