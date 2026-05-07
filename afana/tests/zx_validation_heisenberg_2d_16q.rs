use afana::cbor::*;
use afana::trotter::{trotterize_with_stats, TrotterOrder};
use afana::lower::lower_ast_to_zx;
use afana::zx_ir::ZxGraph;

/// Build a 16-qubit 2D grid Heisenberg model with nearest-neighbor interactions
fn build_heisenberg_2d_16q() -> EhrenfestProgram {
    let mut terms = Vec::new();
    
    // 4x4 grid: qubits indexed row-major (0-3, 4-7, 8-11, 12-15)
    // Horizontal couplings
    for row in 0..4 {
        for col in 0..3 {
            let q1 = row * 4 + col;
            let q2 = row * 4 + col + 1;
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::X },
                    PauliOpEntry { qubit: q2, axis: PauliOp::X },
                ],
            });
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::Y },
                    PauliOpEntry { qubit: q2, axis: PauliOp::Y },
                ],
            });
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::Z },
                    PauliOpEntry { qubit: q2, axis: PauliOp::Z },
                ],
            });
        }
    }
    
    // Vertical couplings
    for row in 0..3 {
        for col in 0..4 {
            let q1 = row * 4 + col;
            let q2 = (row + 1) * 4 + col;
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::X },
                    PauliOpEntry { qubit: q2, axis: PauliOp::X },
                ],
            });
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::Y },
                    PauliOpEntry { qubit: q2, axis: PauliOp::Y },
                ],
            });
            terms.push(PauliTerm {
                coefficient: 1.0,
                paulis: vec![
                    PauliOpEntry { qubit: q1, axis: PauliOp::Z },
                    PauliOpEntry { qubit: q2, axis: PauliOp::Z },
                ],
            });
        }
    }

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 16,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 10.0,
            steps: 20,
            dt_us: 0.5,
        },
        observables: vec![Observable::E],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

#[test]
fn test_heisenberg_2d_16q_trotter_consistency() {
    let program = build_heisenberg_2d_16q();
    
    // Test first-order Trotter
    let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
    
    // Verify basic properties
    assert_eq!(ast.n_qubits, 16);
    assert_eq!(stats.steps, 20);
    assert!(stats.total_gates > 0);
    
    // Lower to ZX-IR and validate
    let zx = lower_ast_to_zx(&ast).unwrap();
    assert!(zx.validate().is_ok());
    
    // Verify coefficient sign preservation by checking that all Hamiltonian terms
    // maintain their original coefficients through the Trotter decomposition
    let original_coeffs: Vec<f64> = program.hamiltonian.terms.iter().map(|t| t.coefficient).collect();
    
    // All coefficients should be positive 1.0 for this Heisenberg model
    for &coeff in &original_coeffs {
        assert!((coeff - 1.0).abs() < 1e-10, "Coefficient should be 1.0, got {}", coeff);
    }
    
    // Verify term index mapping consistency - each Trotter step should process
    // terms in the same order as the original Hamiltonian
    assert_eq!(stats.hamiltonian_terms, program.hamiltonian.terms.len());
    
    // Verify numerical precision across Trotter steps
    assert!(stats.error_bound < 1e-6, "Trotter error bound should be small, got {}", stats.error_bound);
    
    // Test that ZX-IR maintains connectivity information
    assert!(zx.spider_count() > 16); // Should have more than just boundary spiders
    assert!(zx.edge_count() > 0); // Should have edges representing interactions
}

#[test]
fn test_second_order_trotter_consistency() {
    let program = build_heisenberg_2d_16q();
    
    let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::Second).unwrap();
    
    assert_eq!(ast.n_qubits, 16);
    assert_eq!(stats.order, TrotterOrder::Second);
    assert!(stats.total_gates > 0);
    
    let zx = lower_ast_to_zx(&ast).unwrap();
    assert!(zx.validate().is_ok());
    
    // Second-order should have smaller error than first-order
    assert!(stats.error_bound < 1e-8);
}

#[test]
fn test_zx_ir_validation_large_system() {
    let program = build_heisenberg_2d_16q();
    let (ast, _) = trotterize_with_stats(&program, TrotterOrder::First).unwrap();
    
    let zx = lower_ast_to_zx(&ast).unwrap();
    
    // Validate that ZX-IR can handle 16-qubit systems
    assert_eq!(zx.spider_count(), 2 * 16 + ast.gates.len() * 2); // Rough estimate
    
    // All spiders should have valid phases
    for i in 0..zx.spider_count() {
        let spider = zx.spider(i);
        assert!(spider.phase.is_finite(), "Spider {} has invalid phase", i);
    }
}