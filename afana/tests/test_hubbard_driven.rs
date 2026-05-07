// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Test time-dependent driven Hubbard model ZX-IR generation and validation.

use afana::cbor;
use afana::lower;
use afana::trotter;
use afana::type_check;

#[test]
fn test_hubbard_driven_cbor_deserialization() {
    // Load the CBOR hex for time-dependent driven Hubbard model
    let hex = include_str!("../../spec/examples/hubbard_driven_4q.cbor.hex");
    let bytes = hex::decode(hex.trim()).expect("valid hex");
    
    // Test that CBOR deserializes correctly
    let program = cbor::from_cbor(&bytes).expect("CBOR deserialization should succeed");
    
    assert_eq!(program.version, 1);
    assert_eq!(program.system.n_qubits, 4);
    assert_eq!(program.evolution.steps, 100);
    assert_eq!(program.evolution.total_us, 60.0);
    assert_eq!(program.evolution.dt_us, 0.6);
    assert_eq!(program.observables.len(), 2);
}

#[test]
fn test_hubbard_driven_zx_ir_validation() {
    // Load and deserialize the CBOR
    let hex = include_str!("../../spec/examples/hubbard_driven_4q.cbor.hex");
    let bytes = hex::decode(hex.trim()).expect("valid hex");
    let program = cbor::from_cbor(&bytes).expect("CBOR deserialization should succeed");
    
    // Trotterize to get AST
    let (ast, _stats) = trotter::trotterize_with_stats(&program, trotter::TrotterOrder::First)
        .expect("Trotterization should succeed");
    
    // Type check the AST
    type_check::type_check_ast(&ast).expect("Type checking should pass");
    
    // Lower to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
    
    // Validate the ZX-IR representation
    let validation_result = zx_graph.validate();
    assert!(validation_result.is_ok(), "ZX-IR validation should pass: {:?}", validation_result.err());
    
    // Verify the graph has reasonable structure
    assert!(zx_graph.spider_count() > 4, "Should have more than just boundary spiders");
    assert!(zx_graph.edge_count() > 0, "Should have edges between spiders");
}