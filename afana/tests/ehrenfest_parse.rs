use afana::cbor::{
    EhrenfestProgram, EvolutionTime, Hamiltonian, NoiseConstraint, Observable, PauliOp,
    PauliOpEntry, PauliTerm, SystemDef,
};

/// Build a minimal single-qubit Ehrenfest program for testing.
fn make_single_qubit_program() -> EhrenfestProgram {
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 1,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![
                PauliTerm {
                    coefficient: 1.5,
                    paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
                },
                PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
                },
            ],
            constant_offset: -0.25,
        },
        evolution: EvolutionTime {
            total_us: 2.0,
            steps: 20,
            dt_us: 0.1,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: Some(0.99),
            readout_fidelity_min: None,
        },
    }
}

#[test]
fn ehrenfest_deserialization() {
    // Serialize a single-qubit Hamiltonian program to CBOR.
    let program = make_single_qubit_program();
    let mut buf = Vec::new();
    ciborium::into_writer(&program, &mut buf).expect("serialization should succeed");

    // Deserialize from CBOR bytes.
    let decoded = afana::cbor::from_cbor(&buf).expect("deserialization should succeed");

    // Verify top-level fields.
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.system.n_qubits, 1);

    // Verify Hamiltonian term extraction.
    assert_eq!(decoded.hamiltonian.terms.len(), 2);
    assert_eq!(decoded.hamiltonian.constant_offset, -0.25);

    // First term: 1.5 * X_0
    assert!((decoded.hamiltonian.terms[0].coefficient - 1.5).abs() < 1e-10);
    assert_eq!(decoded.hamiltonian.terms[0].paulis.len(), 1);
    assert_eq!(decoded.hamiltonian.terms[0].paulis[0].qubit, 0);
    assert_eq!(decoded.hamiltonian.terms[0].paulis[0].axis, PauliOp::X);

    // Second term: 0.5 * Z_0
    assert!((decoded.hamiltonian.terms[1].coefficient - 0.5).abs() < 1e-10);
    assert_eq!(decoded.hamiltonian.terms[1].paulis.len(), 1);
    assert_eq!(decoded.hamiltonian.terms[1].paulis[0].qubit, 0);
    assert_eq!(decoded.hamiltonian.terms[1].paulis[0].axis, PauliOp::Z);

    // Verify evolution parameters.
    assert_eq!(decoded.evolution.steps, 20);
    assert!((decoded.evolution.total_us - 2.0).abs() < 1e-10);
    assert!((decoded.evolution.dt_us - 0.1).abs() < 1e-10);

    // Verify observables.
    assert_eq!(decoded.observables.len(), 1);
    assert_eq!(decoded.observables[0], Observable::SZ { qubit: 0 });

    // Verify noise constraints.
    assert!((decoded.noise.t1_us - 100.0).abs() < 1e-10);
    assert!((decoded.noise.t2_us - 50.0).abs() < 1e-10);
}
