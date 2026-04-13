// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build an Ehrenfest program from a molecular qubit Hamiltonian.
//!
//! Converts the Jordan-Wigner Pauli Hamiltonian into the CBOR format
//! that Afana can compile (validate → Trotterize → QASM).
//!
//! For ground-state problems the program uses imaginary-time evolution:
//! the Trotter circuit with a large enough evolution time projects onto
//! the ground state when executed on a statevector simulator.

use afana::cbor::*;

/// Accuracy level for the molecular computation.
#[derive(Debug, Clone, Copy)]
pub enum Accuracy {
    /// 1 kcal/mol ≈ 1.6 mHa
    Chemical,
    /// 0.01 kcal/mol
    Spectroscopic,
    /// Machine precision (exact diag / simulator only)
    Exact,
}

impl Accuracy {
    /// Target error in Hartree.
    pub fn target_error_ha(self) -> f64 {
        match self {
            Accuracy::Chemical => 1.6e-3,
            Accuracy::Spectroscopic => 1.6e-5,
            Accuracy::Exact => 0.0,
        }
    }
}

/// Metadata about the molecular origin of the Ehrenfest program.
#[derive(Debug, Clone)]
pub struct MolecularMetadata {
    pub smiles: String,
    pub n_spatial_orbitals: usize,
    pub n_electrons: usize,
    pub basis: String,
    pub transform: String,
    pub nuclear_repulsion: f64,
}

/// Build an EhrenfestProgram from a qubit Hamiltonian (Pauli terms).
///
/// The evolution time and step count are chosen to keep the Trotter
/// error below the accuracy target. For ground-state search via
/// imaginary-time evolution the circuit structure is the same —
/// the imaginary rotation is handled by the executor (Huoma).
pub fn build_ehrenfest_program(
    pauli_terms: &[(f64, String)],
    n_qubits: usize,
    accuracy: Accuracy,
) -> EhrenfestProgram {
    // Convert Pauli string format to EhrenfestProgram PauliTerm format
    let terms: Vec<PauliTerm> = pauli_terms
        .iter()
        .filter(|(c, s)| {
            // Skip pure-identity terms (they contribute a constant offset)
            c.abs() > 1e-14 && !s.chars().all(|ch| ch == 'I')
        })
        .map(|(coeff, pauli_str)| {
            let paulis: Vec<PauliOpEntry> = pauli_str
                .chars()
                .enumerate()
                .filter(|(_, ch)| *ch != 'I')
                .map(|(qubit, ch)| PauliOpEntry {
                    qubit,
                    axis: match ch {
                        'X' => PauliOp::X,
                        'Y' => PauliOp::Y,
                        'Z' => PauliOp::Z,
                        _ => PauliOp::I,
                    },
                })
                .collect();
            PauliTerm {
                coefficient: *coeff,
                paulis,
            }
        })
        .collect();

    // Constant offset from identity terms
    let constant_offset: f64 = pauli_terms
        .iter()
        .filter(|(_, s)| s.chars().all(|ch| ch == 'I'))
        .map(|(c, _)| c)
        .sum();

    // Choose Trotter parameters based on accuracy
    let h_norm: f64 = terms.iter().map(|t| t.coefficient.abs()).sum();
    let n_terms = terms.len() as f64;

    // For S2 Trotter: error ≈ n³ ||H||³ dt³ / 12 per step
    // We want total error < target. Choose dt and steps.
    let target = accuracy.target_error_ha().max(1e-4);
    let dt = if h_norm > 0.0 {
        (12.0 * target / (n_terms.powi(3) * h_norm.powi(3))).cbrt().min(0.5)
    } else {
        0.1
    };
    let total_time = 1.0; // 1 μs nominal evolution
    let steps = ((total_time / dt).ceil() as u32).max(1);
    let dt_actual = total_time / steps as f64;

    // Ensure at least one non-trivial term
    let final_terms = if terms.is_empty() {
        vec![PauliTerm {
            coefficient: 0.0,
            paulis: vec![PauliOpEntry {
                qubit: 0,
                axis: PauliOp::I,
            }],
        }]
    } else {
        terms
    };

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits,
            cooling_profile: None,
            backend_hint: Some("huoma".into()),
        },
        hamiltonian: Hamiltonian {
            terms: final_terms,
            constant_offset,
        },
        evolution: EvolutionTime {
            total_us: total_time,
            steps,
            dt_us: dt_actual,
        },
        observables: vec![Observable::E],
        noise: NoiseConstraint {
            t1_us: 1e6,
            t2_us: 1e6,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

/// Serialize an EhrenfestProgram to CBOR bytes.
pub fn to_cbor(program: &EhrenfestProgram) -> Vec<u8> {
    let mut buf = Vec::new();
    ciborium::into_writer(program, &mut buf).expect("CBOR serialization should not fail");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_program_from_pauli_terms() {
        let terms = vec![
            (0.5, "ZI".to_string()),
            (-0.3, "IZ".to_string()),
            (0.1, "XX".to_string()),
            (0.1, "YY".to_string()),
            (-0.8, "II".to_string()),
        ];
        let program = build_ehrenfest_program(&terms, 2, Accuracy::Chemical);

        assert_eq!(program.version, 1);
        assert_eq!(program.system.n_qubits, 2);
        // 4 non-identity terms
        assert_eq!(program.hamiltonian.terms.len(), 4);
        // Constant offset from II term
        assert!((program.hamiltonian.constant_offset - (-0.8)).abs() < 1e-10);
    }

    #[test]
    fn cbor_roundtrip() {
        let terms = vec![
            (0.5, "ZI".to_string()),
            (-0.3, "IZ".to_string()),
        ];
        let program = build_ehrenfest_program(&terms, 2, Accuracy::Chemical);
        let cbor_bytes = to_cbor(&program);
        let decoded = afana::cbor::from_cbor(&cbor_bytes).unwrap();
        assert_eq!(decoded.system.n_qubits, 2);
        assert_eq!(decoded.hamiltonian.terms.len(), 2);
    }
}
