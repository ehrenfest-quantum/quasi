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
    /// Total CCX (Toffoli) gates.
    pub ccx_count: usize,
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
    let mut ccx_count = 0;

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
            GateName::Ccx => {
                entangling_gates.push(gate.clone());
                ccx_count += 1;
            }
            _ => {}
        }
    }

    // Pass 2: Detect H-CX-H → CZ patterns.
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

    // Pass 3: Detect Toffoli decomposition patterns → synthesize CCX.
    let toffoli_gates = synthesize_toffoli_from_gates(gates);
    for tg in &toffoli_gates {
        entangling_gates.push(tg.clone());
        ccx_count += 1;
    }

    SynthesisResult {
        entangling_gates,
        cx_count,
        cz_count,
        ccx_count,
    }
}

/// Synthesize Toffoli (CCX) gates from a gate sequence by detecting the
/// standard decomposition pattern.
///
/// The standard Toffoli decomposition into Clifford+T gates is:
/// ```text
/// H target
/// CX ctrl2, target
/// Tdg target
/// CX ctrl1, target
/// T target
/// CX ctrl2, target
/// Tdg target
/// CX ctrl1, target
/// T ctrl2
/// T target
/// H target
/// CX ctrl1, ctrl2
/// T ctrl1
/// Tdg ctrl2
/// CX ctrl1, ctrl2
/// ```
///
/// This function scans for this 15-gate pattern and returns synthesized
/// CCX gates with the identified control and target qubits.
pub fn synthesize_toffoli_from_gates(gates: &[Gate]) -> Vec<Gate> {
    let mut result = Vec::new();
    let len = gates.len();
    if len < 15 {
        return result;
    }

    let mut i = 0;
    while i + 14 < len {
        if let Some(ccx) = try_match_toffoli_pattern(&gates[i..i + 15]) {
            result.push(ccx);
            i += 15;
        } else {
            i += 1;
        }
    }

    result
}

/// Try to match the standard 15-gate Toffoli decomposition at the given slice.
///
/// Returns `Some(Gate)` with `GateName::Ccx` if the pattern matches, identifying
/// ctrl1, ctrl2, and target qubits from the decomposition structure.
fn try_match_toffoli_pattern(g: &[Gate]) -> Option<Gate> {
    if g.len() != 15 {
        return None;
    }

    // Gate 0: H target
    if g[0].name != GateName::H || g[0].qubits.len() != 1 {
        return None;
    }
    let target = g[0].qubits[0];

    // Gate 1: CX ctrl2, target
    if g[1].name != GateName::Cx || g[1].qubits.len() != 2 || g[1].qubits[1] != target {
        return None;
    }
    let ctrl2 = g[1].qubits[0];

    // Gate 2: Tdg target
    if g[2].name != GateName::Tdg || g[2].qubits != [target] {
        return None;
    }

    // Gate 3: CX ctrl1, target
    if g[3].name != GateName::Cx || g[3].qubits.len() != 2 || g[3].qubits[1] != target {
        return None;
    }
    let ctrl1 = g[3].qubits[0];

    // ctrl1 and ctrl2 must be different qubits, and neither can be target.
    if ctrl1 == ctrl2 || ctrl1 == target || ctrl2 == target {
        return None;
    }

    // Gate 4: T target
    if g[4].name != GateName::T || g[4].qubits != [target] {
        return None;
    }

    // Gate 5: CX ctrl2, target
    if g[5].name != GateName::Cx || g[5].qubits != [ctrl2, target] {
        return None;
    }

    // Gate 6: Tdg target
    if g[6].name != GateName::Tdg || g[6].qubits != [target] {
        return None;
    }

    // Gate 7: CX ctrl1, target
    if g[7].name != GateName::Cx || g[7].qubits != [ctrl1, target] {
        return None;
    }

    // Gate 8: T ctrl2
    if g[8].name != GateName::T || g[8].qubits != [ctrl2] {
        return None;
    }

    // Gate 9: T target
    if g[9].name != GateName::T || g[9].qubits != [target] {
        return None;
    }

    // Gate 10: H target
    if g[10].name != GateName::H || g[10].qubits != [target] {
        return None;
    }

    // Gate 11: CX ctrl1, ctrl2
    if g[11].name != GateName::Cx || g[11].qubits != [ctrl1, ctrl2] {
        return None;
    }

    // Gate 12: T ctrl1
    if g[12].name != GateName::T || g[12].qubits != [ctrl1] {
        return None;
    }

    // Gate 13: Tdg ctrl2
    if g[13].name != GateName::Tdg || g[13].qubits != [ctrl2] {
        return None;
    }

    // Gate 14: CX ctrl1, ctrl2
    if g[14].name != GateName::Cx || g[14].qubits != [ctrl1, ctrl2] {
        return None;
    }

    Some(Gate {
        name: GateName::Ccx,
        qubits: vec![ctrl1, ctrl2, target],
        params: vec![],
    })
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
        assert_eq!(result.ccx_count, 0);
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
        assert_eq!(result.ccx_count, 0);
    }

    #[test]
    fn explicit_ccx_detected() {
        let gates = vec![
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
        ];
        let result = synthesize_entangling_gates(&gates);
        assert_eq!(result.cx_count, 0);
        assert_eq!(result.cz_count, 0);
        assert_eq!(result.ccx_count, 1);
        assert_eq!(result.entangling_gates.len(), 1);
        assert_eq!(result.entangling_gates[0].name, GateName::Ccx);
        assert_eq!(result.entangling_gates[0].qubits, vec![0, 1, 2]);
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

    /// Build the standard 15-gate Toffoli decomposition for ctrl1, ctrl2, target.
    fn toffoli_decomposition(ctrl1: usize, ctrl2: usize, target: usize) -> Vec<Gate> {
        vec![
            Gate { name: GateName::H,   qubits: vec![target],        params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl2, target], params: vec![] },
            Gate { name: GateName::Tdg, qubits: vec![target],        params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl1, target], params: vec![] },
            Gate { name: GateName::T,   qubits: vec![target],        params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl2, target], params: vec![] },
            Gate { name: GateName::Tdg, qubits: vec![target],        params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl1, target], params: vec![] },
            Gate { name: GateName::T,   qubits: vec![ctrl2],         params: vec![] },
            Gate { name: GateName::T,   qubits: vec![target],        params: vec![] },
            Gate { name: GateName::H,   qubits: vec![target],        params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl1, ctrl2],  params: vec![] },
            Gate { name: GateName::T,   qubits: vec![ctrl1],         params: vec![] },
            Gate { name: GateName::Tdg, qubits: vec![ctrl2],         params: vec![] },
            Gate { name: GateName::Cx,  qubits: vec![ctrl1, ctrl2],  params: vec![] },
        ]
    }

    #[test]
    fn toffoli_decomposition_synthesized_to_ccx() {
        let gates = toffoli_decomposition(0, 1, 2);
        let result = synthesize_toffoli_from_gates(&gates);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, GateName::Ccx);
        assert_eq!(result[0].qubits, vec![0, 1, 2]);
    }

    #[test]
    fn toffoli_synthesis_different_qubits() {
        // ctrl1=3, ctrl2=5, target=7 — non-contiguous qubits.
        let gates = toffoli_decomposition(3, 5, 7);
        let result = synthesize_toffoli_from_gates(&gates);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].qubits, vec![3, 5, 7]);
    }

    #[test]
    fn toffoli_in_larger_sequence() {
        // Pad with unrelated gates before and after.
        let mut gates = vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::X, qubits: vec![1], params: vec![] },
        ];
        gates.extend(toffoli_decomposition(0, 1, 2));
        gates.push(Gate { name: GateName::H, qubits: vec![0], params: vec![] });

        let result = synthesize_entangling_gates(&gates);
        assert_eq!(result.ccx_count, 1);
        // The decomposition also has 6 explicit CX gates.
        assert!(result.cx_count >= 6);
    }

    #[test]
    fn toffoli_not_matched_with_wrong_qubits() {
        // Break the pattern: gate 5 uses wrong control qubit.
        let mut gates = toffoli_decomposition(0, 1, 2);
        gates[5] = Gate { name: GateName::Cx, qubits: vec![0, 2], params: vec![] }; // wrong: should be ctrl2=1
        let result = synthesize_toffoli_from_gates(&gates);
        assert!(result.is_empty());
    }

    #[test]
    fn synthesized_ccx_emits_qasm3() {
        // Build an AST with an explicit CCX gate, verify QASM emission.
        let ast = EhrenfestAst {
            name: "toffoli_test".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![
                Gate { name: GateName::H, qubits: vec![0], params: vec![] },
                Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
            ],
            measures: vec![
                Measure { qubit: 0, cbit: 0 },
                Measure { qubit: 1, cbit: 1 },
                Measure { qubit: 2, cbit: 2 },
            ],
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("ccx q[0], q[1], q[2];"), "QASM3 must contain ccx statement");
        assert!(qasm.contains("OPENQASM 3.0;"));
    }

    #[test]
    fn synthesized_ccx_emits_qasm2() {
        let ast = EhrenfestAst {
            name: "toffoli_v2".into(),
            n_qubits: 3,
            prepare: None,
            gates: vec![
                Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
            ],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("ccx q[0], q[1], q[2];"), "QASM2 must contain ccx statement");
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
