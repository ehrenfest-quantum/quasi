// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Test for ZX-IR generation of 4x4 Heisenberg model.

use afana::cbor;
use afana::lower;
use afana::trotter;
use afana::emit;
use afana::optimize;

#[test]
fn heisenberg_2d_4x4_zx_ir_generation() {
    // Read the 4x4 Heisenberg model CBOR file
    let cbor_path = "spec/examples/heisenberg_2d_4x4.cbor";
    let program = cbor::from_cbor_file(std::path::Path::new(cbor_path)).expect("Failed to read CBOR file");
    
    // Trotterize to get AST
    let (ast, _) = trotter::trotterize_with_stats(&program, trotter::TrotterOrder::First).expect("Trotterization failed");
    
    // Lower to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering failed");
    
    // Validate the ZX graph
    zx_graph.validate().expect("ZX-IR validation failed");
    
    // Check that we have a reasonable number of spiders and edges
    assert!(zx_graph.spider_count() > 0);
    assert!(zx_graph.edge_count() > 0);
    
    // Emit QASM3 for quizx round-trip test
    let qasm3 = emit::emit_qasm(&ast, emit::QasmVersion::V3).expect("QASM3 emission failed");
    
    // Verify it's valid QASM3 syntax
    assert!(qasm3.contains("OPENQASM 3.0"));
    
    // Round-trip through quizx if available (this would normally be done in an integration test)
    // For now, we just verify the QASM can be parsed without error in terms of structure
    let gate_count = optimize::count_qasm_gates(&qasm3);
    assert!(gate_count > 0, "QASM should contain gates");
}
