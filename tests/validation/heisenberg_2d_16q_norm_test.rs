#[test]
fn heisenberg_2d_16q_normalization() {
    // Construct a minimal Hamiltonian with correctly normalized terms.
    use afana::cbor::{Hamiltonian, PauliTerm};
    use afana::validation::validate_hamiltonian_normalization;

    let term = PauliTerm {
        coefficient: 1.0,
        paulis: vec![], // empty paulis list represents an identity term; sufficient for normalization test
    };
    let hamiltonian = Hamiltonian {
        terms: vec![term],
        constant_offset: 0.0,
    };

    // Should succeed for normalized coefficient.
    assert!(validate_hamiltonian_normalization(&hamiltonian).is_ok());

    // Create a Hamiltonian with an unnormalized coefficient to ensure validation catches it.
    let bad_term = PauliTerm {
        coefficient: 0.5,
        paulis: vec![],
    };
    let bad_hamiltonian = Hamiltonian {
        terms: vec![bad_term],
        constant_offset: 0.0,
    };
    assert!(validate_hamiltonian_normalization(&bad_hamiltonian).is_err());
}
