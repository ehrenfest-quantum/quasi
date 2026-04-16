// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Core IR data structures for flattened Pauli-sum representation.

/// A single Pauli term: coefficient times a tensor product of Pauli operators.
#[derive(Debug, Clone, PartialEq)]
pub struct PauliTerm {
    /// Coefficient (real scalar multiplier).
    pub coefficient: f64,
    /// Pauli operators as (qubit, axis) pairs; axis: 0=I, 1=X, 2=Y, 3=Z.
    pub paulis: Vec<(usize, u8)>,
}

impl PauliTerm {
    /// Create a new Pauli term.
    pub fn new(coefficient: f64, paulis: Vec<(usize, u8)>) -> Self {
        Self { coefficient, paulis }
    }
}

/// A sum of Pauli terms (Hamiltonian or observable).
#[derive(Debug, Clone, PartialEq)]
pub struct PauliSum {
    /// List of Pauli terms.
    pub terms: Vec<PauliTerm>,
    /// Constant energy offset.
    pub constant_offset: f64,
}

impl PauliSum {
    /// Create a new Pauli sum.
    pub fn new(terms: Vec<PauliTerm>, constant_offset: f64) -> Self {
        Self { terms, constant_offset }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauli_term_new() {
        let term = PauliTerm::new(0.5, vec![(0, 3), (1, 1)]); // 0.5 * Z0 X1
        assert_eq!(term.coefficient, 0.5);
        assert_eq!(term.paulis, vec![(0, 3), (1, 1)]);
    }

    #[test]
    fn pauli_sum_new() {
        let terms = vec![PauliTerm::new(1.0, vec![(0, 3)]), PauliTerm::new(0.5, vec![(1, 1)])];
        let sum = PauliSum::new(terms.clone(), 0.25);
        assert_eq!(sum.terms, terms);
        assert_eq!(sum.constant_offset, 0.25);
    }
}