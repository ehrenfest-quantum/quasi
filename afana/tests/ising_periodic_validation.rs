//! Tests for ZX-IR validation of transverse-field Ising model with periodic boundary conditions.

use afana::cbor::*;
use afana::trotter::*;
use afana::lower::*;

/// Create an 8-qubit transverse-field Ising model with periodic boundary conditions.
/// H = -J Σᵢ ZᵢZᵢ₊₁ - h Σᵢ Xᵢ with periodic boundaries (Z₈Z₁ term included).
fn ising_8q_periodic() -> EhrenfestProgram {
    let n_qubits = 8;
    let j_coeff = -1.0; // ZZ coupling
    let h_coeff = -0.5; // Transverse field
    
    let mut terms = Vec::new();
    
    // ZZ terms with periodic boundary conditions
    for i in 0..n_qubits {
        let next = (i + 1) % n_qubits; // Periodic boundary
        terms.push(PauliTerm {
            coefficient: j_coeff,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: next, axis: PauliOp::Z },
            ],
        });
    }
    
    // X terms (transverse field)
    for i in 0..n_qubits {
        terms.push(PauliTerm {
            coefficient: h_coeff,
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

#[test]
fn test_ising_periodic_boundary() {
    let program = ising_8q_periodic();
    
    // Validate the Hamiltonian
    assert!(validate_hamiltonian(&program).is_ok());
    
    // Trotterize to AST
    let (ast, _) = trotterize_with_stats(&program, TrotterOrder::First).expect("Trotterization failed");
    
    // Lower to ZX-IR
    let zx_graph = lower_ast_to_zx(&ast).expect("ZX-IR lowering failed");
    
    // Validate ZX-IR
    assert!(zx_graph.validate().is_ok(), "ZX-IR validation failed");
    
    // Check that we have the expected number of qubits
    assert_eq!(ast.n_qubits, 8);
    
    // Check that the ZX graph has reasonable size (at least inputs + outputs)
    assert!(zx_graph.spider_count() >= 16); // 8 inputs + 8 outputs
}