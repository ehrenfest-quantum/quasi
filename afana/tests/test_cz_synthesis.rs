// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests for CZ gate synthesis from ZX-IR spiders.

use afana::ast::{EhrenfestAst, Gate, GateName, Measure};
use afana::emit::{emit_qasm, QasmVersion};
use afana::zx_to_qasm::{decompose_spider_pair, Graph, Node, Type};

/// Build a ZX graph representing two adjacent Z-spiders with phase π.
fn cz_spider_graph() -> Graph {
    Graph {
        spider_types: vec![Type::Z, Type::Z],
        phases: vec![std::f64::consts::PI, std::f64::consts::PI],
        edges: vec![vec![1], vec![0]],
    }
}

#[test]
fn test_cz_synthesis_from_z_spiders() {
    let graph = cz_spider_graph();
    let result = decompose_spider_pair(&graph, 0, 1);
    
    assert!(result.is_some(), "Should detect CZ pattern from Z-spiders with phase π");
    let gates = result.unwrap();
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].name, GateName::Cz);
    assert_eq!(gates[0].qubits, vec![0, 1]);
}

#[test]
fn test_cz_qasm3_output() {
    let graph = cz_spider_graph();
    let cz_gates = decompose_spider_pair(&graph, 0, 1).unwrap();
    
    let ast = EhrenfestAst {
        name: "cz_integration".into(),
        n_qubits: 2,
        prepare: None,
        gates: cz_gates,
        measures: vec![
            Measure { qubit: 0, cbit: 0 },
            Measure { qubit: 1, cbit: 1 },
        ],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };
    
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    // Verify QASM3 format
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("include \"stdgates.inc\";"));
    assert!(qasm.contains("qubit[2] q;"));
    
    // Verify CZ gate emission
    assert!(qasm.contains("cz q[0], q[1];"), "QASM3 must contain cz statement");
    
    // Verify measurements
    assert!(qasm.contains("c[0] = measure q[0];"));
    assert!(qasm.contains("c[1] = measure q[1];"));
}

#[test]
fn test_cz_qasm2_output() {
    let graph = cz_spider_graph();
    let cz_gates = decompose_spider_pair(&graph, 0, 1).unwrap();
    
    let ast = EhrenfestAst {
        name: "cz_integration".into(),
        n_qubits: 2,
        prepare: None,
        gates: cz_gates,
        measures: vec![
            Measure { qubit: 0, cbit: 0 },
            Measure { qubit: 1, cbit: 1 },
        ],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };
    
    let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
    
    // Verify QASM2 format
    assert!(qasm.contains("OPENQASM 2.0;"));
    assert!(qasm.contains("include \"qelib1.inc\";"));
    assert!(qasm.contains("qreg q[2];"));
    
    // Verify CZ gate emission
    assert!(qasm.contains("cz q[0], q[1];"), "QASM2 must contain cz statement");
}

#[test]
fn test_cz_with_hadamard_preparation() {
    // Test CZ in context of a typical circuit (H on control, then CZ)
    let graph = cz_spider_graph();
    let cz_gates = decompose_spider_pair(&graph, 0, 1).unwrap();
    
    let mut gates = vec![
        Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        },
    ];
    gates.extend(cz_gates);
    
    let ast = EhrenfestAst {
        name: "cz_with_h".into(),
        n_qubits: 2,
        prepare: None,
        gates,
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };
    
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("cz q[0], q[1];"));
}

#[test]
fn test_non_pi_phase_no_cz() {
    let mut graph = cz_spider_graph();
    graph.phases[0] = std::f64::consts::FRAC_PI_2; // π/2 instead of π
    
    let result = decompose_spider_pair(&graph, 0, 1);
    assert!(result.is_none(), "Should not synthesize CZ for non-π phase");
}

#[test]
fn test_x_spider_no_cz() {
    let mut graph = cz_spider_graph();
    graph.spider_types[0] = Type::X;
    
    let result = decompose_spider_pair(&graph, 0, 1);
    assert!(result.is_none(), "Should not synthesize CZ for X spider");
}

#[test]
fn test_disconnected_spiders_no_cz() {
    let mut graph = cz_spider_graph();
    graph.edges = vec![vec![], vec![]]; // No edges between spiders
    
    let result = decompose_spider_pair(&graph, 0, 1);
    assert!(result.is_none(), "Should not synthesize CZ for disconnected spiders");
}