use crate::cbor::{Hamiltonian, PauliTerm};

/// Validates that each Hamiltonian term is properly normalized.
/// For the Heisenberg model we expect each term coefficient to be exactly 1.0 (within a small epsilon).
pub fn validate_hamiltonian_normalization(hamiltonian: &Hamiltonian) -> Result<(), String> {
    const EPS: f64 = 1e-6;
    for term in &hamiltonian.terms {
        if (term.coefficient - 1.0).abs() > EPS {
            return Err(format!(
                "Hamiltonian term coefficient {} is not normalized to 1.0",
                term.coefficient
            ));
        }
    }
    Ok(())
}
