// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Observable measurement synthesis.
//!
//! Takes the observable list from an Ehrenfest program and synthesizes the
//! measurement circuits needed — basis-change gates and measurement directives.

use crate::ast::{EhrenfestAst, Gate, GateName, Measure};
use crate::cbor::Observable;

// ── Public types ───────────────────────────────────────────────────────────

/// A measurement prescription for a single observable.
#[derive(Debug, Clone)]
pub struct MeasurementCircuit {
    /// The observable being measured.
    pub observable: ObservableKind,
    /// Basis-change gates to prepend before measurement.
    pub basis_change_gates: Vec<Gate>,
    /// Which qubits to measure.
    pub measure_qubits: Vec<usize>,
    /// Classical post-processing description.
    pub postprocess: PostProcess,
}

/// Classified observable kind.
#[derive(Debug, Clone)]
pub enum ObservableKind {
    PauliZ { qubit: usize },
    PauliX { qubit: usize },
    Energy,
    DensityMatrix { qubits: Vec<usize> },
    Fidelity { target_state: Vec<u8> },
}

/// Post-processing mode for measurement results.
#[derive(Debug, Clone, PartialEq)]
pub enum PostProcess {
    /// Direct measurement result (Z-basis).
    DirectMeasurement,
    /// Expectation value from measurement statistics.
    ExpectationValue,
    /// Full state tomography needed.
    Tomography { n_qubits: usize },
}

// ── Public functions ───────────────────────────────────────────────────────

/// Synthesize measurement circuits for a list of observables.
///
/// For each observable, produces the basis-change gates and measurement
/// qubits needed:
/// - **SZ**: No basis change, measure qubit directly.
/// - **SX**: H gate rotates X→Z basis before measurement.
/// - **E (Energy)**: Measure all qubits in Z basis.
/// - **Density**: Measure specified qubits, needs tomography.
/// - **F (Fidelity)**: Measure all qubits, compare to target.
pub fn synthesize_measurements(
    observables: &[Observable],
    n_qubits: usize,
) -> Vec<MeasurementCircuit> {
    let mut circuits = Vec::new();

    for obs in observables {
        let circuit = match obs {
            Observable::SZ { qubit } => MeasurementCircuit {
                observable: ObservableKind::PauliZ { qubit: *qubit },
                basis_change_gates: Vec::new(),
                measure_qubits: vec![*qubit],
                postprocess: PostProcess::DirectMeasurement,
            },

            Observable::SX { qubit } => MeasurementCircuit {
                observable: ObservableKind::PauliX { qubit: *qubit },
                basis_change_gates: vec![Gate {
                    name: GateName::H,
                    qubits: vec![*qubit],
                    params: vec![],
                }],
                measure_qubits: vec![*qubit],
                postprocess: PostProcess::ExpectationValue,
            },

            Observable::E => MeasurementCircuit {
                observable: ObservableKind::Energy,
                basis_change_gates: Vec::new(),
                measure_qubits: (0..n_qubits).collect(),
                postprocess: PostProcess::ExpectationValue,
            },

            Observable::Density { qubits } => MeasurementCircuit {
                observable: ObservableKind::DensityMatrix {
                    qubits: qubits.clone(),
                },
                basis_change_gates: Vec::new(),
                measure_qubits: qubits.clone(),
                postprocess: PostProcess::Tomography {
                    n_qubits: qubits.len(),
                },
            },

            Observable::F { target_state } => MeasurementCircuit {
                observable: ObservableKind::Fidelity {
                    target_state: target_state.clone(),
                },
                basis_change_gates: Vec::new(),
                measure_qubits: (0..n_qubits).collect(),
                postprocess: PostProcess::ExpectationValue,
            },
        };
        circuits.push(circuit);
    }

    circuits
}

/// Apply measurement circuits to an AST in place.
///
/// Appends basis-change gates to `ast.gates` and adds measurement
/// instructions to `ast.measures`. Classical bit indices are assigned
/// sequentially starting after any existing measurements.
pub fn apply_measurement_circuits(ast: &mut EhrenfestAst, circuits: &[MeasurementCircuit]) {
    // Find the next available classical bit index.
    let mut next_cbit = ast.measures.iter().map(|m| m.cbit + 1).max().unwrap_or(0);

    for circuit in circuits {
        // Append basis-change gates.
        ast.gates.extend(circuit.basis_change_gates.clone());

        // Append measurements.
        for &qubit in &circuit.measure_qubits {
            ast.measures.push(Measure {
                qubit,
                cbit: next_cbit,
            });
            next_cbit += 1;
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ast(n_qubits: usize) -> EhrenfestAst {
        EhrenfestAst {
            name: "test".into(),
            n_qubits,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        }
    }

    #[test]
    fn sz_no_basis_change_measure_one_qubit() {
        let observables = vec![Observable::SZ { qubit: 2 }];
        let circuits = synthesize_measurements(&observables, 4);
        assert_eq!(circuits.len(), 1);
        assert!(circuits[0].basis_change_gates.is_empty());
        assert_eq!(circuits[0].measure_qubits, vec![2]);
        assert_eq!(circuits[0].postprocess, PostProcess::DirectMeasurement);
    }

    #[test]
    fn sx_one_h_gate_prepended() {
        let observables = vec![Observable::SX { qubit: 1 }];
        let circuits = synthesize_measurements(&observables, 4);
        assert_eq!(circuits.len(), 1);
        assert_eq!(circuits[0].basis_change_gates.len(), 1);
        assert_eq!(circuits[0].basis_change_gates[0].name, GateName::H);
        assert_eq!(circuits[0].basis_change_gates[0].qubits, vec![1]);
        assert_eq!(circuits[0].measure_qubits, vec![1]);
        assert_eq!(circuits[0].postprocess, PostProcess::ExpectationValue);
    }

    #[test]
    fn energy_measures_all_qubits_no_basis_change() {
        let observables = vec![Observable::E];
        let circuits = synthesize_measurements(&observables, 3);
        assert_eq!(circuits.len(), 1);
        assert!(circuits[0].basis_change_gates.is_empty());
        assert_eq!(circuits[0].measure_qubits, vec![0, 1, 2]);
        assert_eq!(circuits[0].postprocess, PostProcess::ExpectationValue);
    }

    #[test]
    fn multiple_observables_all_collected() {
        let observables = vec![
            Observable::SZ { qubit: 0 },
            Observable::SX { qubit: 1 },
            Observable::E,
        ];
        let circuits = synthesize_measurements(&observables, 2);
        assert_eq!(circuits.len(), 3);
        // SX should have basis change, others should not.
        assert!(circuits[0].basis_change_gates.is_empty());
        assert_eq!(circuits[1].basis_change_gates.len(), 1);
        assert!(circuits[2].basis_change_gates.is_empty());
    }

    #[test]
    fn density_requires_tomography() {
        let observables = vec![Observable::Density {
            qubits: vec![0, 2],
        }];
        let circuits = synthesize_measurements(&observables, 4);
        assert_eq!(circuits.len(), 1);
        assert_eq!(circuits[0].measure_qubits, vec![0, 2]);
        assert_eq!(
            circuits[0].postprocess,
            PostProcess::Tomography { n_qubits: 2 }
        );
    }

    #[test]
    fn fidelity_measures_all_qubits() {
        let observables = vec![Observable::F {
            target_state: vec![0, 1],
        }];
        let circuits = synthesize_measurements(&observables, 2);
        assert_eq!(circuits.len(), 1);
        assert_eq!(circuits[0].measure_qubits, vec![0, 1]);
        assert_eq!(circuits[0].postprocess, PostProcess::ExpectationValue);
    }

    #[test]
    fn apply_measurement_circuits_modifies_ast() {
        let mut ast = empty_ast(3);
        // Pre-existing gate.
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        });

        let observables = vec![Observable::SX { qubit: 1 }, Observable::SZ { qubit: 0 }];
        let circuits = synthesize_measurements(&observables, 3);
        apply_measurement_circuits(&mut ast, &circuits);

        // Should have original H + basis-change H for SX = 2 gates.
        assert_eq!(ast.gates.len(), 2);
        assert_eq!(ast.gates[1].name, GateName::H);
        assert_eq!(ast.gates[1].qubits, vec![1]);

        // Should have 2 measurements: qubit 1 (SX) and qubit 0 (SZ).
        assert_eq!(ast.measures.len(), 2);
        assert_eq!(ast.measures[0].qubit, 1);
        assert_eq!(ast.measures[0].cbit, 0);
        assert_eq!(ast.measures[1].qubit, 0);
        assert_eq!(ast.measures[1].cbit, 1);
    }

    #[test]
    fn apply_preserves_existing_measures() {
        let mut ast = empty_ast(3);
        ast.measures.push(Measure { qubit: 2, cbit: 5 });

        let observables = vec![Observable::SZ { qubit: 0 }];
        let circuits = synthesize_measurements(&observables, 3);
        apply_measurement_circuits(&mut ast, &circuits);

        // Should have original measure + new one.
        assert_eq!(ast.measures.len(), 2);
        // New cbit should start after existing max (5+1=6).
        assert_eq!(ast.measures[1].cbit, 6);
    }
}
