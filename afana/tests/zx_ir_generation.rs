// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Tests for ZX-IR generation of XXZ spin chain models.

use afana::cbor::*;
use afana::lower::lower_ast_to_zx;
use afana::trotter::{trotterize, TrotterOrder};
use afana::zx_ir::SpiderColor;

/// Create an XXZ spin chain Hamiltonian with anisotropy parameter δ.
/// 
/// H = Σⱼ Jₓₓ(XⱼXⱼ₊₁ + YⱼYⱼ₊₁) + J_zz(ZⱼZⱼ₊₁)
/// With J_xx = 1.0 and J_zz = δ (anisotropy parameter)
fn xxz_hamiltonian(n_qubits: usize, delta: f64) -> EhrenfestProgram {
    let mut terms = Vec::new();
    
    // XX and YY terms (J_xx = 1.0)
    for j in 0..n_qubits-1 {
        // XX term: X_j X_{j+1}
        terms.push(PauliTerm {
            coefficient: 1.0,
            paulis: vec![
                PauliOpEntry { qubit: j, axis: PauliOp::X },
                PauliOpEntry { qubit: j+1, axis: PauliOp::X },
            ],
        });
        
        // YY term: Y_j Y_{j+1}
        terms.push(PauliTerm {
            coefficient: 1.0,
            paulis: vec![
                PauliOpEntry { qubit: j, axis: PauliOp::Y },
                PauliOpEntry { qubit: j+1, axis: PauliOp::Y },
            ],
        });
    }
    
    // ZZ terms with anisotropy parameter δ
    for j in 0..n_qubits-1 {
        terms.push(PauliTerm {
            coefficient: delta,
            paulis: vec![
                PauliOpEntry { qubit: j, axis: PauliOp::Z },
                PauliOpEntry { qubit: j+1, axis: PauliOp::Z },
            ],
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
        observables: vec![],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

#[test]
fn xxz_12q_zx_ir_generation() {
    // Test 12-qubit XXZ chain with anisotropy parameter δ = 1.0
    let program = xxz_hamiltonian(12, 1.0);
    
    // Trotterize to get AST
    let ast = trotterize(&program, TrotterOrder::First);
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
    
    // Validate the graph structure
    zx_graph.validate().expect("ZX graph should be valid");
    
    // Check node count: 12 input boundary + 12 output boundary + gate spiders
    // For XXZ model we expect many internal spiders from the Trotter decomposition
    assert_eq!(zx_graph.inputs.len(), 12);
    assert_eq!(zx_graph.outputs.len(), 12);
    
    // Check that we have a reasonable number of spiders (more than just boundaries)
    assert!(zx_graph.spider_count() > 24, "Should have internal spiders from XXZ terms");
    
    // Check that all input/output nodes are Z-spiders with phase 0
    for &input_node in &zx_graph.inputs {
        let spider = zx_graph.spider(input_node);
        assert_eq!(spider.color, SpiderColor::Z);
        assert!((spider.phase - 0.0).abs() < 1e-10);
    }
    
    for &output_node in &zx_graph.outputs {
        let spider = zx_graph.spider(output_node);
        assert_eq!(spider.color, SpiderColor::Z);
        assert!((spider.phase - 0.0).abs() < 1e-10);
    }
}

#[test]
fn xxz_zx_ir_with_anisotropy_parameter_range() {
    // Test that different anisotropy parameters produce valid ZX-IR
    for &delta in &[0.5, 1.0, 1.5, 2.0] {
        let program = xxz_hamiltonian(4, delta);
        let ast = trotterize(&program, TrotterOrder::First);
        let zx_graph = lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
        zx_graph.validate().expect("ZX graph should be valid for delta={delta}");
        
        // Check basic structure
        assert_eq!(zx_graph.inputs.len(), 4);
        assert_eq!(zx_graph.outputs.len(), 4);
    }
}

#[test]
fn xxz_zx_ir_with_small_time_step() {
    // Test with small time step dt ≤ 0.1 as required
    let mut program = xxz_hamiltonian(6, 1.0);
    program.evolution.dt_us = 0.1;
    program.evolution.total_us = 1.0;
    program.evolution.steps = 10; // dt = total/steps = 0.1
    
    let ast = trotterize(&program, TrotterOrder::First);
    let zx_graph = lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
    zx_graph.validate().expect("ZX graph should be valid with small time step");
    
    // Should have more Trotter steps → more internal structure
    assert!(zx_graph.spider_count() > 12*2); // More than just input/output boundaries
}
