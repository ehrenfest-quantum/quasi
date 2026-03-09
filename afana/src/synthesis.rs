// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Entangling gate synthesis from gate sequences.
//!
//! Detects patterns in gate sequences that correspond to entangling operations
//! (CNOT/CX and CZ) and synthesizes them into explicit two-qubit gates.
//!
//! This module bridges the gap between Trotterized gate sequences (which use
//! CNOT ladders implicitly) and explicit entangling gate representation needed
//! for ZX-IR analysis in future phases.

use crate::ast::{Gate, GateName};

/// Result of entangling gate synthesis analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct SynthesisResult {
    /// Entangling gates found or synthesized.
    pub entangling_gates: Vec<Gate>,
    /// Total CX (CNOT) gates.
    pub cx_count: usize,
    /// Total CZ gates.
    pub cz_count: usize,
}

/// Analyze a gate sequence and extract/synthesize entangling gates.
///
/// Identifies two patterns:
/// 1. **Explicit CX/CZ gates** already present in the sequence
/// 2. **CZ patterns**: `H target; CX control,target; H target` → equivalent CZ
///
/// Returns a [`SynthesisResult`] summarizing the entangling structure.
pub fn synthesize_entangling_gates(gates: &[Gate]) -> SynthesisResult {
    let mut entangling_gates = Vec::new();
    let mut cx_count = 0;
    let mut cz_count = 0;

    // Pass 1: Collect explicit entangling gates.
    for gate in gates {
        match gate.name {
            GateName::Cx => {
                entangling_gates.push(gate.clone());
                cx_count += 1;
            }
            GateName::Cz => {
                entangling_gates.push(gate.clone());
                cz_count += 1;
            }
            _ => {}
        }
    }

    // Pass 2: Detect H-CX-H → CZ patterns.
    // (existing code unchanged)
    // Pass 3: Detect simple Toffoli pattern: Cx c1,t; Cx c2,t; Cx c1,t → synthesize Ccx.
    // This pattern assumes the first and third Cx share the same control and target,
    // and the middle Cx shares the same target with a different control.
    // When detected, we emit a three‑qubit CCX (Toffoli) gate.
    i = 0;
    while i + 2 < gates.len() {
        let g0 = &gates[i];
        let g1 = &gates[i + 1];
        let g2 = &gates[i + 2];
        if g0.name == GateName::Cx && g2.name == GateName::Cx && g1.name == GateName::Cx {
            // Ensure g0 and g2 are identical (same control and target)
            if g0.qubits == g2.qubits {
                let target = g0.qubits[1];
                let ctrl1 = g0.qubits[0];
                let ctrl2 = g1.qubits[0];
                // g1 must target the same qubit
                if g1.qubits[1] == target && ctrl2 != ctrl1 {
                    // Synthesize CCX gate
                    let ccx = Gate {
                        name: GateName::Ccx,
                        qubits: vec![ctrl1, ctrl2, target],
                        params: vec![],
                    };
                    entangling_gates.push(ccx);
                    // Skip the three Cx gates we just consumed
                    i += 3;
                    continue;
                }
            }
        }
        i += 1;
    }

    // Pattern: H on target, then CX(control, target), then H on target.
    let mut i = 0;
    while i + 2 < gates.len() {
        if gates[i].name == GateName::H
            && gates[i].qubits.len() == 1
            && gates[i + 1].name == GateName::Cx
            && gates[i + 1].qubits.len() == 2
            && gates[i + 2].name == GateName::H
            && gates[i + 2].qubits.len() == 1
        {
            let h_qubit = gates[i].qubits[0];
            let cx_target = gates[i + 1].qubits[1];
            let h2_qubit = gates[i + 2].qubits[0];

            if h_qubit == cx_target && h2_qubit == cx_target {
                // H-CX-H pattern on target qubit → synthesized CZ.
                let cz = Gate {
                    name: GateName::Cz,
                    qubits: gates[i + 1].qubits.clone(),
                    params: vec![],
                };
                entangling_gates.push(cz);
                cz_count += 1;
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    SynthesisResult {
        entangling_gates,
        cx_count,
        cz_count,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::cbor::*;
    use crate::emit::{emit_qasm, QasmVersion};
    use crate::trotter::{trotterize, TrotterOrder};

    #[test]
    fn explicit_cx_detected() {
        let gates = vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ];
        let result = synthesize_entangling_gates(&gates);
        assert_eq!(result.cx_count, 1);
        assert_eq!(result.cz_count, 0);
        assert_eq!(result.entangling_gates.len(), 1);
        assert_eq!(result.entangling_gates[0].name, GateName::Cx);
    }

    #[test]
    fn explicit_cz_detected() {
        let gates = vec![
            Gate { name: GateName::Cz, qubits: vec![0, 1], params: vec![] },
        ];
        let result = synthesize_entangling_gates(&gates);
        assert_eq!(result.cx_count, 0);
        assert_eq!(result.cz_count, 1);
    }

    #[test]
    fn h_cx_h_pattern_synthesizes_cz() {
        let gates = vec![
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
        ];
        let result = synthesize_entangling_gates(&gates);
        // The explicit CX is counted, plus the synthesized CZ from the pattern.
        assert_eq!(result.cx_count, 1);
        assert_eq!(result.cz_count, 1);
    }

    #[test]
    fn no_entangling_gates_in_single_qubit() {
        let gates = vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![1.0] },
        ];
        let result = synthesize_entangling_gates(&gates);
        assert_eq!(result.cx_count, 0);
        assert_eq!(result.cz_count, 0);
        assert!(result.entangling_gates.is_empty());
    }

    #[test]
    fn trotterized_zz_contains_cnot_gates() {
        // ZZ Hamiltonian produces CNOT ladder via Trotterization.
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
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Z },
                    ],
                }],
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
        };

        let ast = trotterize(&program, TrotterOrder::First);
        let result = synthesize_entangling_gates(&ast.gates);
        assert!(result.cx_count >= 2, "ZZ term needs CNOT ladder (at least 2 CX gates)");

        // Verify QASM output contains cx gates.
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("cx"), "QASM output must contain cx gates");
    }

    #[test]
    fn trotterized_xx_contains_cnot_and_produces_qasm() {
        // XX Hamiltonian: H-CX-Rz-CX-H pattern.
        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 2,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient: 0.3,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::X },
                        PauliOpEntry { qubit: 1, axis: PauliOp::X },
                    ],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 1,
                dt_us: 1.0,
            },
            observables: vec![Observable::SX { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };

        let ast = trotterize(&program, TrotterOrder::First);
        let result = synthesize_entangling_gates(&ast.gates);
        assert!(result.cx_count >= 2, "XX term needs CNOT ladder");

        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("cx"), "QASM3 output must contain cx");
        assert!(qasm.contains("h "), "QASM3 output must contain h gates for X basis");
    }

    #[test]
    fn multi_qubit_heisenberg_contains_entangling_gates() {
        // XYZ Heisenberg model: 3 terms, each producing entangling gates.
        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 3,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![
                    PauliTerm {
                        coefficient: 0.5,
                        paulis: vec![
                            PauliOpEntry { qubit: 0, axis: PauliOp::X },
                            PauliOpEntry { qubit: 1, axis: PauliOp::X },
                        ],
                    },
                    PauliTerm {
                        coefficient: 0.5,
                        paulis: vec![
                            PauliOpEntry { qubit: 1, axis: PauliOp::Y },
                            PauliOpEntry { qubit: 2, axis: PauliOp::Y },
                        ],
                    },
                    PauliTerm {
                        coefficient: 0.5,
                        paulis: vec![
                            PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                            PauliOpEntry { qubit: 2, axis: PauliOp::Z },
                        ],
                    },
                ],
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
        };

        let ast = trotterize(&program, TrotterOrder::First);
        let result = synthesize_entangling_gates(&ast.gates);

        // All 3 terms are 2-qubit → each produces at least 2 CX gates.
        assert!(result.cx_count >= 6, "3 two-qubit terms → at least 6 CX gates, got {}", result.cx_count);

        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("cx"), "Heisenberg QASM must contain cx");
    }
}
