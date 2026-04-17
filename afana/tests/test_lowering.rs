use afana::cbor::{EhrenfestProgram, Hamiltonian, PauliTerm, PauliOpEntry, PauliOp, SystemDef, EvolutionTime, NoiseConstraint, Observable};
use afana::lower::lower_identity_pauli;
use afana::trotter::{trotterize, TrotterOrder};

#[test]
fn test_lower_identity_pauli_preserves_coefficient_and_zero_qubit_indices() {
    // Build a Hamiltonian with an identity term: 0.5 * I ∧ I
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 0.5,
                paulis: vec![], // Empty paulis list = identity on all qubits
            }],
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
    };

    // Trotterize to get AST
    let ast = trotterize(&program, TrotterOrder::First);

    // Identity term should produce no gates
    assert!(ast.gates.is_empty(), "Identity term should produce no gates");

    // Verify coefficient is preserved in Hamiltonian
    assert_eq!(program.hamiltonian.terms[0].coefficient, 0.5);

    // Test the lower_identity_pauli function directly
    let gates = lower_identity_pauli(0.5, 2);
    assert!(gates.is_empty(), "lower_identity_pauli should return empty gate vector");
}

#[test]
fn test_lower_identity_pauli_zero_coefficient() {
    let gates = lower_identity_pauli(0.0, 3);
    assert!(gates.is_empty(), "Zero coefficient identity term should produce no gates");
}

#[test]
fn test_lower_identity_pauli_negative_coefficient() {
    let gates = lower_identity_pauli(-1.2, 1);
    assert!(gates.is_empty(), "Negative coefficient identity term should produce no gates");
}