// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests for gate synthesis from ZX-IR patterns.

use afana::ast::{Gate, GateName};
use afana::emit::{emit_qasm, QasmVersion};
use afana::synthesis::zx_to_qasm::{QasmProgram, Spider, SpiderType, synthesize_from_zx};

/// Test that CH gate is correctly synthesized from ZX-IR Hadamard spider pattern
/// and emitted as valid QASM3 'ch' statement.
#[test]
fn test_ch_gate_synthesis_from_zx_spiders() {
    // Create ZX graph with CH pattern:
    // - Z spider on qubit 0 (control)
    // - Hadamard spider on qubit 1 (target)
    // Connected by edge, representing controlled-Hadamard
    let spiders = vec![
        Spider {
            node_type: SpiderType::Z,
            phase: 0.0,
            qubit: 0,
            neighbors: vec![1],
        },
        Spider {
            node_type: SpiderType::Hadamard,
            phase: 0.0,
            qubit: 1,
            neighbors: vec![0],
        },
    ];

    // Synthesize from ZX-IR
    let program = synthesize_from_zx(&spiders);

    // Verify synthesis produced CH gate
    assert_eq!(program.n_qubits(), 2, "Should have 2 qubits");
    assert_eq!(program.gates().len(), 1, "Should produce exactly one gate");
    let ch_gate = &program.gates()[0];
    assert_eq!(ch_gate.name, GateName::Ch, "Gate should be CH");
    assert_eq!(ch_gate.qubits, vec![0, 1], "CH should be on qubits 0 (control) and 1 (target)");
    assert!(ch_gate.params.is_empty(), "CH gate should have no parameters");

    // Build AST for QASM emission
    let ast = afana::ast::EhrenfestAst {
        name: "ch_test".into(),
        n_qubits: 2,
        prepare: None,
        gates: program.gates().to_vec(),
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    // Emit QASM3 and verify 'ch' statement is present
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM emission should succeed");
    assert!(qasm.contains("ch q[0], q[1];"), "QASM3 output should contain 'ch' statement");
    assert!(qasm.contains("OPENQASM 3.0;"), "Should be QASM3");
    assert!(qasm.contains("include \"stdgates.inc\";"), "Should include stdgates");
}

/// Test that CH gate is correctly emitted in QASM2 format as well
#[test]
fn test_ch_gate_qasm2_emission() {
    let spiders = vec![
        Spider {
            node_type: SpiderType::Z,
            phase: 0.0,
            qubit: 0,
            neighbors: vec![1],
        },
        Spider {
            node_type: SpiderType::Hadamard,
            phase: 0.0,
            qubit: 1,
            neighbors: vec![0],
        },
    ];

    let program = synthesize_from_zx(&spiders);
    let ast = afana::ast::EhrenfestAst {
        name: "ch_test_v2".into(),
        n_qubits: 2,
        prepare: None,
        gates: program.gates().to_vec(),
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let qasm = emit_qasm(&ast, QasmVersion::V2).expect("QASM2 emission should succeed");
    assert!(qasm.contains("ch q[0], q[1];"), "QASM2 output should contain 'ch' statement");
    assert!(qasm.contains("OPENQASM 2.0;"), "Should be QASM2");
}

/// Test that single Hadamard spider produces H gate (not CH)
#[test]
fn test_single_hadamard_spider_produces_h_gate() {
    let spiders = vec![Spider {
        node_type: SpiderType::Hadamard,
        phase: 0.0,
        qubit: 0,
        neighbors: vec![],
    }];

    let program = synthesize_from_zx(&spiders);
    assert_eq!(program.gates().len(), 1);
    assert_eq!(program.gates()[0].name, GateName::H);
}
