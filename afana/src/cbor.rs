// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! CBOR deserialization for Ehrenfest binary programs.
//!
//! The physics-level `.ef` format is a CBOR document conforming to the schema
//! in `spec/ehrenfest-v0.1.cddl`. This module deserializes it into typed Rust
//! structs that represent the Hamiltonian, observables, and noise constraints.
//!
//! These structs are the input to Afana's Trotterization pass, which derives
//! a gate sequence and produces an [`EhrenfestAst`] for QASM emission.

use serde::{Deserialize, Serialize};

use crate::error::CborError;

// ── CBOR schema types ────────────────────────────────────────────────────────

/// A Pauli operator: I, X, Y, or Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauliOp {
    I,
    X,
    Y,
    Z,
}

/// A single Pauli term in a Hamiltonian: coefficient × tensor product of Pauli operators.
///
/// Example: `0.5 * Z ⊗ Z` on qubits 0 and 1 → `PauliTerm { coeff: 0.5, ops: [(0, Z), (1, Z)] }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PauliTerm {
    /// Coefficient in GHz·rad units.
    pub coeff: f64,
    /// Qubit index → Pauli operator. Qubits not listed are implicitly I.
    pub ops: Vec<(usize, PauliOp)>,
}

/// Hamiltonian expressed as a sum of Pauli terms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hamiltonian {
    pub terms: Vec<PauliTerm>,
}

/// Evolution time and Trotter decomposition parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionTime {
    /// Total evolution time in natural units.
    pub time: f64,
    /// Number of Trotter steps.
    pub trotter_steps: u32,
    /// Trotter order (1 or 2).
    pub trotter_order: u32,
}

/// Noise constraints — compile-time type check against hardware capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseConstraint {
    /// Minimum T1 relaxation time in microseconds.
    pub t1_us: Option<f64>,
    /// Minimum T2 dephasing time in microseconds.
    pub t2_us: Option<f64>,
    /// Minimum single-gate fidelity.
    pub gate_fidelity_min: Option<f64>,
}

/// An observable to measure after time evolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observable {
    pub name: String,
    pub terms: Vec<PauliTerm>,
}

/// Top-level Ehrenfest binary program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EhrenfestProgram {
    pub version: u32,
    pub system: SystemDef,
    pub hamiltonian: Hamiltonian,
    pub evolution: EvolutionTime,
    pub observables: Vec<Observable>,
    pub noise: Option<NoiseConstraint>,
}

/// Physical system definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemDef {
    pub n_qubits: usize,
}

// ── Deserialization ──────────────────────────────────────────────────────────

/// Deserialize an Ehrenfest binary program from CBOR bytes.
pub fn from_cbor(bytes: &[u8]) -> Result<EhrenfestProgram, CborError> {
    let program: EhrenfestProgram =
        ciborium::from_reader(bytes).map_err(|e| CborError::Decode(e.to_string()))?;

    // Schema validation.
    if program.version != 1 {
        return Err(CborError::Schema(format!(
            "unsupported Ehrenfest version: {} (expected 1)",
            program.version
        )));
    }
    if program.system.n_qubits == 0 {
        return Err(CborError::Schema("n_qubits must be >= 1".into()));
    }
    if program.evolution.trotter_order != 1 && program.evolution.trotter_order != 2 {
        return Err(CborError::Schema(format!(
            "trotter_order must be 1 or 2, got {}",
            program.evolution.trotter_order
        )));
    }

    Ok(program)
}

/// Deserialize an Ehrenfest binary program from a `.ef` file.
pub fn from_cbor_file(path: &std::path::Path) -> Result<EhrenfestProgram, CborError> {
    let bytes = std::fs::read(path)?;
    from_cbor(&bytes)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_program() -> EhrenfestProgram {
        EhrenfestProgram {
            version: 1,
            system: SystemDef { n_qubits: 2 },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coeff: 0.5,
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                }],
            },
            evolution: EvolutionTime {
                time: 1.0,
                trotter_steps: 10,
                trotter_order: 1,
            },
            observables: vec![Observable {
                name: "ZZ".into(),
                terms: vec![PauliTerm {
                    coeff: 1.0,
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                }],
            }],
            noise: Some(NoiseConstraint {
                t1_us: Some(100.0),
                t2_us: Some(50.0),
                gate_fidelity_min: Some(0.99),
            }),
        }
    }

    #[test]
    fn cbor_roundtrip() {
        let program = make_test_program();

        // Serialize to CBOR.
        let mut buf = Vec::new();
        ciborium::into_writer(&program, &mut buf).unwrap();

        // Deserialize back.
        let decoded = from_cbor(&buf).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.system.n_qubits, 2);
        assert_eq!(decoded.hamiltonian.terms.len(), 1);
        assert_eq!(decoded.evolution.trotter_steps, 10);
    }

    #[test]
    fn cbor_rejects_bad_version() {
        let mut program = make_test_program();
        program.version = 99;

        let mut buf = Vec::new();
        ciborium::into_writer(&program, &mut buf).unwrap();

        let err = from_cbor(&buf).unwrap_err();
        assert!(err.to_string().contains("unsupported Ehrenfest version"));
    }
}
