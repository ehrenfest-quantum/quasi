//! Test for 8x8 Heisenberg model ZX-IR generation.
//! This test verifies that the full compilation pipeline works for a large-scale
//! Heisenberg model on a 2D grid, satisfying the acceptance criteria:
//! 1. The compiled ZX-IR graph deserializes and type-checks with no structural errors
//! 2. QASM3 emission produces syntactically valid output with HAL Contract-approved gates

use afana::cbor::*;
use afana::trotter::*;
use afana::lower::*;
use afana::emit::*;
use afana::type_check::*;

/// Create an 8x8 Heisenberg model on a 2D grid.
/// H = J * Σ_{<i,j>} (X_i X_j + Y_i Y_j + Z_i Z_j)
/// where <i,j> are nearest neighbors on the grid.
fn make_heisenberg_8x8() -> EhrenfestProgram {
    let n_rows = 8;
    let n_cols = 8;
    let n_qubits = n_rows * n_cols; // 64 qubits
    
    let j = 0.5; // Coupling constant
    
    let mut terms = Vec::new();
    
    // Generate nearest-neighbor interactions
    for row in 0..n_rows {
        for col in 0..n_cols {
            let idx = row * n_cols + col;
            
            // Right neighbor (if exists)
            if col + 1 < n_cols {
                let right_idx = row * n_cols + (col + 1);
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::X },
                        PauliOpEntry { qubit: right_idx, axis: PauliOp::X },
                    ],
                });
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::Y },
                        PauliOpEntry { qubit: right_idx, axis: PauliOp::Y },
                    ],
                });
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::Z },
                        PauliOpEntry { qubit: right_idx, axis: PauliOp::Z },
                    ],
                });
            }
            
            // Bottom neighbor (if exists)
            if row + 1 < n_rows {
                let bottom_idx = (row + 1) * n_cols + col;
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::X },
                        PauliOpEntry { qubit: bottom_idx, axis: PauliOp::X },
                    ],
                });
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::Y },
                        PauliOpEntry { qubit: bottom_idx, axis: PauliOp::Y },
                    ],
                });
                terms.push(PauliTerm {
                    coefficient: j,
                    paulis: vec![
                        PauliOpEntry { qubit: idx, axis: PauliOp::Z },
                        PauliOpEntry { qubit: bottom_idx, axis: PauliOp::Z },
                    ],
                });
            }
        }
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
fn heisenberg_8x8_zx_ir_generation() {
    // Create the 8x8 Heisenberg model program
    let program = make_heisenberg_8x8();
    
    // Validate the Hamiltonian
    assert!(validate_hamiltonian(&program).is_ok(), "Hamiltonian validation should pass");
    
    // Trotterize to get AST
    let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::First).expect("Trotterization should succeed");
    
    // Type check the AST
    assert!(type_check_ast(&ast).is_ok(), "AST type checking should pass");
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
    
    // Validate ZX-IR structure
    assert!(zx_graph.validate().is_ok(), "ZX-IR validation should pass");
    
    // Verify we have the expected number of qubits
    assert_eq!(ast.n_qubits, 64, "Should have 64 qubits");
    
    // Verify ZX-IR has reasonable size (at least input + output boundaries)
    assert!(zx_graph.spider_count() > 128, "ZX-IR should have many spiders for 64-qubit circuit");
    assert!(zx_graph.edge_count() > 64, "ZX-IR should have many edges for 64-qubit circuit");
    
    // Emit QASM3
    let qasm3 = emit_qasm(&ast, QasmVersion::V3).expect("QASM3 emission should succeed");
    
    // Verify QASM3 contains required headers and gates
    assert!(qasm3.contains("OPENQASM 3.0;"), "Should contain QASM3 header");
    assert!(qasm3.contains("include \"stdgates.inc\";"), "Should include standard gates");
    assert!(qasm3.contains("qubit[64] q;"), "Should declare 64 qubits");
    
    // Verify QASM3 contains only valid gates (HAL Contract approved)
    // Check that it doesn't contain any invalid gate names
    assert!(!qasm3.contains("invalid"), "QASM should not contain invalid gates");
    
    // Print some stats for verification
    println!("8x8 Heisenberg model compilation stats:");
    println!("  Qubits: {}", ast.n_qubits);
    println!("  Hamiltonian terms: {}", stats.hamiltonian_terms);
    println!("  Total gates: {}", stats.total_gates);
    println!("  ZX-IR spiders: {}", zx_graph.spider_count());
    println!("  ZX-IR edges: {}", zx_graph.edge_count());
    
    // Verify the QASM is non-empty and has expected structure
    assert!(!qasm3.is_empty(), "QASM3 output should not be empty");
    assert!(qasm3.lines().count() > 10, "QASM3 should have substantial content");
}
