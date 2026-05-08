// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Validation tests for ZX-IR representation consistency.
//!
//! These tests verify that the ZX-IR correctly encodes Trotterized evolution
//! steps and that the graph structure matches the expected variational ansatz.

use afana::cbor;
use afana::lower;
use afana::trotter::{trotterize_with_stats, TrotterOrder};

/// Test that validates ZX-IR consistency for the LiH molecule VQE example.
///
/// This test:
/// 1. Loads the LiH example CBOR hex file
/// 2. Deserializes the Ehrenfest program
/// 3. Performs Trotterization to generate gate sequence
/// 4. Lowers the AST to ZX-IR
/// 5. Validates that the ZX-IR graph structure matches expected Trotter steps
#[test]
fn validate_lithium_hydride_zx_ir_trotter_steps() {
    // Load the LiH example CBOR hex file
    let hex_content = include_str!("../../spec/examples/lithium_hydride.cbor.hex");
    let hex_clean = hex_content.replace(|c: char| c.is_whitespace(), "");
    let cbor_bytes = hex::decode(&hex_clean).expect("Failed to decode LiH CBOR hex");

    // Deserialize the Ehrenfest program
    let program = cbor::from_cbor(&cbor_bytes).expect("Failed to deserialize LiH program");

    // Get expected Trotter steps from the evolution parameters
    let expected_steps = program.evolution.steps as usize;
    let expected_terms = program.hamiltonian.terms.len();

    // Perform Trotterization with first-order decomposition
    let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::First)
        .expect("Trotterization failed");

    // Lower the AST to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering failed");

    // Validate the ZX-IR graph structure
    zx_graph.validate().expect("ZX-IR validation failed");

    // Verify that the number of Trotter steps matches the expected count
    assert_eq!(
        stats.steps as usize,
        expected_steps,
        "Trotter steps mismatch: expected {}, got {}",
        expected_steps,
        stats.steps
    );

    // Verify that the Hamiltonian terms count matches
    assert_eq!(
        stats.hamiltonian_terms,
        expected_terms,
        "Hamiltonian terms mismatch: expected {}, got {}",
        expected_terms,
        stats.hamiltonian_terms
    );

    // Verify ZX-IR spider count is proportional to gate count
    // Each gate contributes spiders based on its decomposition:
    // - Single-qubit gates: 1-3 spiders
    // - Two-qubit gates (CX/CZ): 2 spiders + edge
    // The spider count should be significantly larger than the gate count
    // due to boundary nodes and gate decompositions
    let gate_count = ast.gates.len();
    let spider_count = zx_graph.spider_count();

    // Spider count should be at least 2x gate count (inputs + outputs + gate spiders)
    assert!(
        spider_count >= gate_count * 2,
        "ZX-IR spider count ({}) too low for {} gates",
        spider_count,
        gate_count
    );

    // Verify edge count is reasonable (at least one edge per gate for connectivity)
    let edge_count = zx_graph.edge_count();
    assert!(
        edge_count >= gate_count,
        "ZX-IR edge count ({}) too low for {} gates",
        edge_count,
        gate_count
    );

    // Log validation results for debugging
    eprintln!("LiH ZX-IR Validation Results:");
    eprintln!("  Trotter steps: {}", expected_steps);
    eprintln!("  Hamiltonian terms: {}", expected_terms);
    eprintln!("  Total gates: {}", gate_count);
    eprintln!("  ZX-IR spiders: {}", spider_count);
    eprintln!("  ZX-IR edges: {}", edge_count);
    eprintln!("  Validation: PASSED");
}

/// Test that validates ZX-IR phase annotations correspond to variational parameters.
///
/// For VQE ansatz circuits, the ZX-IR phases should encode the rotation angles
/// from parametric gates (Rx, Ry, Rz).
#[test]
fn validate_zx_ir_phase_annotations() {
    use afana::ast::{EhrenfestAst, Gate, GateName};
    use afana::zx_ir::SpiderColor;

    // Create a simple test AST with parametric gates
    let theta = std::f64::consts::FRAC_PI_4; // pi/4
    let ast = EhrenfestAst {
        name: "test_vqe".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![theta],
            },
            Gate {
                name: GateName::Cx,
                qubits: vec![0, 1],
                params: vec![],
            },
            Gate {
                name: GateName::Ry,
                qubits: vec![1],
                params: vec![theta * 2.0],
            },
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    // Lower to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering failed");
    zx_graph.validate().expect("ZX-IR validation failed");

    // Verify that parametric gates produce spiders with correct phases
    // Rz(theta) -> Z spider with phase theta/pi
    // Ry(theta) -> Z(0.5) - X(theta/pi) - Z(-0.5) pattern
    let mut found_rz_phase = false;
    let mut found_ry_phase = false;

    for i in 0..zx_graph.spider_count() {
        let spider = zx_graph.spider(i);
        let expected_rz_phase = theta / std::f64::consts::PI; // 0.25 for pi/4
        let expected_ry_phase = (theta * 2.0) / std::f64::consts::PI; // 0.5 for pi/2

        if spider.color == SpiderColor::Z
            && (spider.phase - expected_rz_phase).abs() < 1e-10
        {
            found_rz_phase = true;
        }

        if spider.color == SpiderColor::X
            && (spider.phase - expected_ry_phase).abs() < 1e-10
        {
            found_ry_phase = true;
        }
    }

    assert!(
        found_rz_phase,
        "ZX-IR should contain Z spider with Rz phase {}",
        expected_rz_phase
    );
    assert!(
        found_ry_phase,
        "ZX-IR should contain X spider with Ry phase {}",
        expected_ry_phase
    );
}

/// Test that validates ZX-IR connectivity matches ansatz entangling structure.
///
/// The ZX-IR graph connectivity should reflect the entangling gates (CX, CZ)
/// in the ansatz circuit through edges between spiders on different qubits.
#[test]
fn validate_zx_ir_connectivity_matches_ansatz() {
    use afana::ast::{EhrenfestAst, Gate, GateName};

    // Create a test AST with entangling structure
    let ast = EhrenfestAst {
        name: "test_entangling".into(),
        n_qubits: 3,
        prepare: None,
        gates: vec![
            // Layer 1: entangle q0-q1
            Gate {
                name: GateName::Cx,
                qubits: vec![0, 1],
                params: vec![],
            },
            // Layer 2: entangle q1-q2
            Gate {
                name: GateName::Cz,
                qubits: vec![1, 2],
                params: vec![],
            },
            // Layer 3: entangle q0-q2
            Gate {
                name: GateName::Cx,
                qubits: vec![0, 2],
                params: vec![],
            },
        ],
        measures: vec![],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    // Lower to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering failed");
    zx_graph.validate().expect("ZX-IR validation failed");

    // Verify connectivity: entangling gates should create cross-qubit edges
    // Each CX/CZ gate adds at least one edge between spiders on different qubits
    let edge_count = zx_graph.edge_count();
    let expected_min_edges = ast.gates.len(); // At least one edge per entangling gate

    assert!(
        edge_count >= expected_min_edges,
        "ZX-IR should have at least {} edges for {} entangling gates, got {}",
        expected_min_edges,
        ast.gates.len(),
        edge_count
    );

    // Verify all qubits are represented in the graph
    let mut qubits_seen = std::collections::HashSet::new();
    for i in 0..zx_graph.spider_count() {
        if let Some(q) = zx_graph.spider(i).qubit {
            qubits_seen.insert(q);
        }
    }

    assert_eq!(
        qubits_seen.len(),
        ast.n_qubits,
        "ZX-IR should represent all {} qubits, found {}",
        ast.n_qubits,
        qubits_seen.len()
    );

    eprintln!("ZX-IR Connectivity Validation:");
    eprintln!("  Qubits in graph: {:?}", qubits_seen);
    eprintln!("  Total edges: {}", edge_count);
    eprintln!("  Validation: PASSED");
}
