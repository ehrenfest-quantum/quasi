// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Test for SX gate emission from ZX-IR to QASM3.
//!
//! This test validates that the SX gate (square root of X) is correctly
//! emitted in QASM3 syntax for hardware compatibility.

use afana::ast::{EhrenfestAst, Gate, GateName};
use afana::emit::{emit_qasm, QasmVersion};

/// Helper: build a simple circuit with an SX gate.
fn sx_circuit_ast() -> EhrenfestAst {
    EhrenfestAst {
        name: "sx_test".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::H,
                qubits: vec![0],
                params: vec![],
            },
            Gate {
                name: GateName::Sx,
                qubits: vec![0],
                params: vec![],
            },
            Gate {
                name: GateName::Sx,
                qubits: vec![1],
                params: vec![],
            },
            Gate {
                name: GateName::Cx,
                qubits: vec![0, 1],
                params: vec![],
            },
        ],
        measures: vec![
            afana::ast::Measure { qubit: 0, cbit: 0 },
            afana::ast::Measure { qubit: 1, cbit: 1 },
        ],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

#[test]
fn test_sx_gate_emission_qasm3() {
    let ast = sx_circuit_ast();
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM3 emission should succeed");

    // Verify QASM3 header
    assert!(qasm.contains("OPENQASM 3.0;"), "Should emit QASM3 header");
    assert!(qasm.contains("include \"stdgates.inc\";"), "Should include stdgates.inc");

    // Verify qubit declaration
    assert!(qasm.contains("qubit[2] q;"), "Should declare 2 qubits");

    // Verify SX gate emission
    assert!(qasm.contains("sx q[0];"), "Should emit SX gate on q[0]");
    assert!(qasm.contains("sx q[1];"), "Should emit SX gate on q[1]");

    // Verify other gates are also present
    assert!(qasm.contains("h q[0];"), "Should emit H gate");
    assert!(qasm.contains("cx q[0], q[1];"), "Should emit CX gate");

    // Verify measurements
    assert!(qasm.contains("c[0] = measure q[0];"), "Should emit measurement for q[0]");
    assert!(qasm.contains("c[1] = measure q[1];"), "Should emit measurement for q[1]");
}

#[test]
fn test_sx_gate_emission_qasm2() {
    let ast = sx_circuit_ast();
    let qasm = emit_qasm(&ast, QasmVersion::V2).expect("QASM2 emission should succeed");

    // Verify QASM2 header
    assert!(qasm.contains("OPENQASM 2.0;"), "Should emit QASM2 header");
    assert!(qasm.contains("include \"qelib1.inc\";"), "Should include qelib1.inc");

    // Verify SX gate emission (QASM2 also supports sx in qelib1.inc)
    assert!(qasm.contains("sx q[0];"), "Should emit SX gate on q[0]");
    assert!(qasm.contains("sx q[1];"), "Should emit SX gate on q[1]");
}

#[test]
fn test_sx_gate_name_parsing() {
    // Test that "sx" token parses correctly
    let gate_name = GateName::from_token("sx");
    assert!(matches!(gate_name, Some(GateName::Sx)), "Should parse 'sx' token");

    // Test case insensitivity
    let gate_name_upper = GateName::from_token("SX");
    assert!(matches!(gate_name_upper, Some(GateName::Sx)), "Should parse 'SX' token");

    // Test as_str returns correct name
    assert_eq!(GateName::Sx.as_str(), "sx", "as_str should return 'sx'");
}

#[test]
fn test_sx_gate_arity() {
    // SX is a single-qubit gate
    assert_eq!(GateName::Sx.arity(), 1, "SX gate should have arity 1");
}

#[test]
fn test_sx_gate_not_parametric() {
    // SX is not a parametric gate
    assert!(!GateName::Sx.is_parametric(), "SX gate should not be parametric");
}

#[test]
fn test_sx_gate_in_sequence() {
    // Test SX gate in a sequence with other gates
    let ast = EhrenfestAst {
        name: "sx_sequence".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::Sx,
                qubits: vec![0],
                params: vec![],
            },
            Gate {
                name: GateName::Sx,
                qubits: vec![0],
                params: vec![],
            },
        ],
        measures: vec![],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("Emission should succeed");

    // Two SX gates in sequence (SX^2 = X)
    let sx_count = qasm.matches("sx q[0];").count();
    assert_eq!(sx_count, 2, "Should emit two SX gates");
}
