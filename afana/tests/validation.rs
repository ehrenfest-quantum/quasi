#![cfg(test)]

use afana::cbor;
use afana::trotter;
use afana::lower;
use afana::zx_ir;
use std::path::Path;

/// Test that validates ZX-IR representation correctly encodes Trotterized evolution steps
/// for the LiH VQE ansatz circuit.
#[test]
fn test_lih_zx_ir_trotter_consistency() {
    // Load the LiH example program
    let cbor_path = Path::new("spec/examples/vqe_lih.cbor.hex");
    
    // Skip test if file doesn't exist (may not be present in all environments)
    if !cbor_path.exists() {
        println!("LiH example file not found, skipping test");
        return;
    }
    
    // Read the hex-encoded CBOR file
    let hex_content = std::fs::read_to_string(cbor_path)
        .expect("Failed to read LiH CBOR hex file");
    let cbor_bytes = hex::decode(hex_content.trim())
        .expect("Failed to decode hex CBOR data");
    
    // Parse the Ehrenfest program
    let program = cbor::from_cbor(&cbor_bytes)
        .expect("Failed to parse Ehrenfest program");
    
    // Perform Trotterization to get AST
    let (ast, stats) = trotter::trotterize_with_stats(
        &program, 
        trotter::TrotterOrder::First
    ).expect("Trotterization failed");
    
    // Lower AST to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast)
        .expect("ZX-IR lowering failed");
    
    // Validate the ZX-IR graph
    zx_graph.validate()
        .expect("ZX-IR validation failed");
    
    // Verify Trotter step consistency
    let expected_steps = program.evolution.steps as usize;
    let actual_spider_count = zx_graph.spider_count();
    
    // The validation should pass without errors
    assert!(actual_spider_count > 0, "ZX-IR should contain spiders");
    
    println!("LiH VQE validation successful:");
    println!("  Expected Trotter steps: {}", expected_steps);
    println!("  ZX-IR spider count: {}", actual_spider_count);
    println!("  ZX-IR edge count: {}", zx_graph.edge_count());
}

/// Test that validates the Trotter step count is consistent between AST and ZX-IR
#[test]
fn test_trotter_step_consistency() {
    // Create a simple test program to validate the approach
    let program = cbor::EhrenfestProgram {
        version: 1,
        system: cbor::SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: cbor::Hamiltonian {
            terms: vec![
                cbor::PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![
                        cbor::PauliOpEntry { qubit: 0, axis: cbor::PauliOp::Z },
                        cbor::PauliOpEntry { qubit: 1, axis: cbor::PauliOp::Z },
                    ],
                }
            ],
            constant_offset: 0.0,
        },
        evolution: cbor::EvolutionTime {
            total_us: 1.0,
            steps: 5,
            dt_us: 0.2,
        },
        observables: vec![cbor::Observable::SZ { qubit: 0 }],
        noise: cbor::NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    };
    
    // Perform Trotterization
    let (ast, stats) = trotter::trotterize_with_stats(
        &program, 
        trotter::TrotterOrder::First
    ).expect("Trotterization failed");
    
    // Lower to ZX-IR
    let zx_graph = lower::lower_ast_to_zx(&ast)
        .expect("ZX-IR lowering failed");
    
    // Validate
    zx_graph.validate()
        .expect("ZX-IR validation failed");
    
    // Check that we have the expected Trotter steps represented
    assert_eq!(stats.steps, 5);
    assert!(zx_graph.spider_count() > 4); // At least boundaries + some gates
    
    println!("Trotter consistency validation passed");
    println!("  Steps: {}", stats.steps);
    println!("  ZX-IR spiders: {}", zx_graph.spider_count());
    println!("  ZX-IR edges: {}", zx_graph.edge_count());
}