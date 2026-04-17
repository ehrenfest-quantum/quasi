//! CI tests for ZX-IR validation.

use afana::zx_ir::{ZxGraph, SpiderColor};
use afana::zx_ir::validate::validate_sign_consistency;

#[test]
fn test_rejects_mixed_sign_pauli_terms() {
    let mut graph = ZxGraph::new();
    
    // Add Pauli terms with mixed signs (+0.5X and -0.5X)
    // First term: +0.5X represented as Z(0) - X(0.5) - Z(0) pattern
    let z1 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    let x1 = graph.add_spider(SpiderColor::X, 0.5, Some(0)); // +0.5 coefficient
    let z2 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    graph.add_edge(z1, x1);
    graph.add_edge(x1, z2);
    
    // Second term: -0.5X represented as Z(0) - X(-0.5) - Z(0) pattern
    let z3 = graph.add_spider(SpiderColor::Z, 0.0, Some(1));
    let x2 = graph.add_spider(SpiderColor::X, -0.5, Some(1)); // -0.5 coefficient
    let z4 = graph.add_spider(SpiderColor::Z, 0.0, Some(1));
    graph.add_edge(z3, x2);
    graph.add_edge(x2, z4);
    
    // Validation should reject mixed signs
    let result = validate_sign_consistency(&graph);
    assert!(result.is_err(), "Graph with mixed-sign Pauli terms should be rejected");
    
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should report validation errors");
    assert!(errors[0].to_string().contains("mixed coefficient signs"), 
            "Error message should indicate mixed signs");
}

#[test]
fn test_accepts_consistent_sign_pauli_terms() {
    let mut graph = ZxGraph::new();
    
    // Add Pauli terms with consistent signs (+0.5X and +0.3X)
    // First term: +0.5X
    let z1 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    let x1 = graph.add_spider(SpiderColor::X, 0.5, Some(0));
    let z2 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    graph.add_edge(z1, x1);
    graph.add_edge(x1, z2);
    
    // Second term: +0.3X
    let z3 = graph.add_spider(SpiderColor::Z, 0.0, Some(1));
    let x2 = graph.add_spider(SpiderColor::X, 0.3, Some(1));
    let z4 = graph.add_spider(SpiderColor::Z, 0.0, Some(1));
    graph.add_edge(z3, x2);
    graph.add_edge(x2, z4);
    
    // Validation should accept consistent signs
    let result = validate_sign_consistency(&graph);
    assert!(result.is_ok(), "Graph with consistent-sign Pauli terms should be accepted");
}

#[test]
fn test_ignores_non_pauli_elements() {
    let mut graph = ZxGraph::new();
    
    // Add some non-Pauli elements that shouldn't affect sign consistency
    // H gate decomposition (Z-X-Z)
    let h_z1 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    let h_x = graph.add_spider(SpiderColor::X, 0.0, Some(0));
    let h_z2 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    graph.add_edge(h_z1, h_x);
    graph.add_edge(h_x, h_z2);
    
    // CX gate (Z-X connection)
    let cx_z = graph.add_spider(SpiderColor::Z, 0.0, Some(1));
    let cx_x = graph.add_spider(SpiderColor::X, 0.0, Some(2));
    graph.add_edge(cx_z, cx_x);
    
    // Now add a Pauli term with positive coefficient
    let z1 = graph.add_spider(SpiderColor::Z, 0.0, Some(3));
    let x1 = graph.add_spider(SpiderColor::X, 0.5, Some(3));
    let z2 = graph.add_spider(SpiderColor::Z, 0.0, Some(3));
    graph.add_edge(z1, x1);
    graph.add_edge(x1, z2);
    
    // Validation should still accept since the single Pauli term has consistent sign
    let result = validate_sign_consistency(&graph);
    assert!(result.is_ok(), "Graph with non-Pauli elements and consistent Pauli signs should be accepted");
}

#[test]
fn test_empty_pauli_terms() {
    let graph = ZxGraph::new();
    
    // Empty graph should pass validation (no Pauli terms to be inconsistent)
    let result = validate_sign_consistency(&graph);
    assert!(result.is_ok(), "Empty graph should be valid");
}

#[test]
fn test_single_pauli_term() {
    let mut graph = ZxGraph::new();
    
    // Add a single Pauli term with negative coefficient
    let z1 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    let x1 = graph.add_spider(SpiderColor::X, -0.5, Some(0));
    let z2 = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    graph.add_edge(z1, x1);
    graph.add_edge(x1, z2);
    
    // Single term should pass regardless of sign
    let result = validate_sign_consistency(&graph);
    assert!(result.is_ok(), "Single Pauli term should be valid");
}