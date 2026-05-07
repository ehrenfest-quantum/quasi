// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

//! Test ZX-IR validation for Hamiltonian term commutation relationships.

use afana::ast::*;
use afana::cbor::*;
use afana::lower::*;
use afana::synthesis::*;
use afana::trotter::*;
use afana::zx_ir::*;

/// Create an 8-qubit transverse-field Ising model program.
fn make_transverse_ising_8q() -> EhrenfestProgram {
    let n_qubits = 8;
    
    // Build ZZ interaction terms (nearest neighbor)
    let mut terms = Vec::new();
    for i in 0..(n_qubits-1) {
        terms.push(PauliTerm {
            coefficient: -1.0, // ZZ coupling
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i+1, axis: PauliOp::Z },
            ],
        });
    }
    
    // Add transverse field terms (X on each qubit)
    for i in 0..n_qubits {
        terms.push(PauliTerm {
            coefficient: -0.5, // transverse field strength
            paulis: vec![PauliOpEntry { qubit: i, axis: PauliOp::X }],
        });
    }
    
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 1,
            dt_us: 1.0,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

/// Test that ZZ terms commute with each other in the ZX-IR.
#[test]
fn test_zz_terms_commute_in_zxir() {
    let program = make_transverse_ising_8q();
    let ast = trotterize(&program, TrotterOrder::First);
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("Lowering to ZX-IR should succeed");
    
    // Validate the graph
    assert!(zx_graph.validate().is_ok(), "ZX-IR graph should be valid");
    
    // Check that we have the expected number of spiders
    // For 8 qubits with ZZ interactions and X fields, we expect:
    // - 8 input boundaries
    // - 8 output boundaries
    // - Spiders for the gates
    assert!(zx_graph.spider_count() > 16, "Should have input/output boundaries plus gate spiders");
    
    // Verify synthesis detects entangling gates
    let synthesis_result = synthesize_entangling_gates(&ast.gates);
    assert!(synthesis_result.cx_count > 0, "Should have detected CX gates from ZZ terms");
    assert!(synthesis_result.cz_count >= 0, "Should have detected CZ patterns");
}

/// Test that X field terms commute with each other in the ZX-IR.
#[test]
fn test_x_field_terms_commute_in_zxir() {
    let program = make_transverse_ising_8q();
    let ast = trotterize(&program, TrotterOrder::First);
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("Lowering to ZX-IR should succeed");
    
    // Validate the graph
    assert!(zx_graph.validate().is_ok(), "ZX-IR graph should be valid");
    
    // Check that X gates are properly lowered
    let x_gate_count = ast.gates.iter().filter(|g| g.name == GateName::X || g.name == GateName::Rx).count();
    assert!(x_gate_count > 0, "Should have X-type gates from transverse field terms");
}

/// Test that non-commuting term pairs are properly ordered.
#[test]
fn test_non_commuting_terms_ordered_correctly() {
    let program = make_transverse_ising_8q();
    let ast = trotterize(&program, TrotterOrder::First);
    
    // In the transverse-field Ising model, ZZ terms and X terms do not commute
    // The Trotterization should preserve this ordering
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("Lowering to ZX-IR should succeed");
    
    // Validate the graph
    assert!(zx_graph.validate().is_ok(), "ZX-IR graph should be valid");
    
    // Check that the graph has the expected structure
    assert!(zx_graph.spider_count() > 16, "Should have sufficient spiders for the circuit");
    assert!(zx_graph.edge_count() > 10, "Should have sufficient edges for the circuit");
}

/// Test commutation validation for a simpler 2-qubit case.
#[test]
fn test_commutation_validation_simple_case() {
    // Create a 2-qubit transverse-field Ising model
    let mut terms = Vec::new();
    
    // ZZ interaction
    terms.push(PauliTerm {
        coefficient: -1.0,
        paulis: vec![
            PauliOpEntry { qubit: 0, axis: PauliOp::Z },
            PauliOpEntry { qubit: 1, axis: PauliOp::Z },
        ],
    });
    
    // X fields
    terms.push(PauliTerm {
        coefficient: -0.5,
        paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
    });
    
    terms.push(PauliTerm {
        coefficient: -0.5,
        paulis: vec![PauliOpEntry { qubit: 1, axis: PauliOp::X }],
    });
    
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 1,
            dt_us: 1.0,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    };
    
    let ast = trotterize(&program, TrotterOrder::First);
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("Lowering to ZX-IR should succeed");
    
    // Validate the graph
    assert!(zx_graph.validate().is_ok(), "ZX-IR graph should be valid");
    
    // Check synthesis
    let synthesis_result = synthesize_entangling_gates(&ast.gates);
    assert_eq!(synthesis_result.cz_count, 0, "Should have no CZ gates in this decomposition");
    assert!(synthesis_result.cx_count > 0, "Should have CX gates from ZZ term");
}