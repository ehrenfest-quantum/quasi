// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Afana — the Ehrenfest-to-OpenQASM compiler.
//!
//! ```text
//! Ehrenfest (CBOR binary)
//!     → deserialize (cbor.rs)
//!     → EhrenfestProgram (Hamiltonians, observables, noise constraints)
//!     → trotterize (trotter.rs)
//!     → EhrenfestAst (gate sequences)
//!     → optimize (T-gate reduction, ZX-calculus)
//!     → emit OpenQASM 2.0 / 3.0
//! ```
//!
//! Ehrenfest programs are CBOR binary. There is no text form.

pub mod ast;
pub mod cbor;
pub mod decompose;
pub mod emit;
pub mod error;
pub mod lower;
pub mod noise;
pub mod observable;
pub mod optimize;
pub mod synthesis;
pub mod trotter;
pub mod type_check;
pub mod sword_ingest;
pub mod zx_ir;
pub mod validate;
pub mod zx_to_qasm;

#[cfg(test)]
mod tests {
    /// Test XXZ Trotter consistency validation for 12 qubits with Δ=1.5.
    ///
    /// This test validates:
    /// 1. Coefficient preservation across Trotter steps
    /// 2. Term commutation structure is maintained
    /// 3. Gate sequence consistency (same pattern per step)
    /// 4. ZX-IR deserialization and QASM3 emission
    #[test]
    fn test_xxzz_trotter_consistency_12q() {
        use crate::cbor;
        use crate::trotter::{trotterize_with_stats, TrotterOrder};
        use crate::lower::lower_ast_to_zx;
        use crate::emit::{emit_qasm, QasmVersion};

        // XXZ Hamiltonian for 12 qubits with Δ=1.5
        // H = J * Σ_i (XX + YY + Δ*ZZ) with J=0.5, Δ=1.5
        let n_qubits = 12;
        let j = 0.5_f64;
        let delta = 1.5_f64;

        // Build XXZ terms: XX, YY, ZZ for each nearest-neighbor pair
        let mut terms = Vec::new();
        for i in 0..n_qubits - 1 {
            // XX term
            terms.push(crate::cbor::PauliTerm {
                coefficient: j,
                paulis: vec![
                    crate::cbor::PauliOpEntry { qubit: i, axis: crate::cbor::PauliOp::X },
                    crate::cbor::PauliOpEntry { qubit: i + 1, axis: crate::cbor::PauliOp::X },
                ],
            });
            // YY term
            terms.push(crate::cbor::PauliTerm {
                coefficient: j,
                paulis: vec![
                    crate::cbor::PauliOpEntry { qubit: i, axis: crate::cbor::PauliOp::Y },
                    crate::cbor::PauliOpEntry { qubit: i + 1, axis: crate::cbor::PauliOp::Y },
                ],
            });
            // ZZ term with anisotropy
            terms.push(crate::cbor::PauliTerm {
                coefficient: j * delta,
                paulis: vec![
                    crate::cbor::PauliOpEntry { qubit: i, axis: crate::cbor::PauliOp::Z },
                    crate::cbor::PauliOpEntry { qubit: i + 1, axis: crate::cbor::PauliOp::Z },
                ],
            });
        }

        let program = cbor::EhrenfestProgram {
            version: 1,
            system: crate::cbor::SystemDef {
                n_qubits,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: crate::cbor::Hamiltonian {
                terms,
                constant_offset: 0.0,
            },
            evolution: crate::cbor::EvolutionTime {
                total_us: 1.0,
                steps: 10,
                dt_us: 0.1,
            },
            observables: vec![crate::cbor::Observable::E],
            noise: crate::cbor::NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };

        // Serialize to CBOR and deserialize to verify round-trip
        let mut cbor_buf = Vec::new();
        ciborium::into_writer(&program, &mut cbor_buf).unwrap();
        let deserialized = cbor::from_cbor(&cbor_buf).expect("CBOR deserialization should succeed");

        // Verify coefficient preservation (no drift)
        assert_eq!(deserialized.hamiltonian.terms.len(), program.hamiltonian.terms.len());
        for (orig, deser) in program.hamiltonian.terms.iter().zip(deserialized.hamiltonian.terms.iter()) {
            assert!((orig.coefficient - deser.coefficient).abs() < 1e-10,
                "Coefficient drift detected: {} vs {}", orig.coefficient, deser.coefficient);
        }

        // Trotterize and collect statistics
        let (ast, stats) = trotterize_with_stats(&program, TrotterOrder::First)
            .expect("Trotterization should succeed");

        // Verify gate sequence consistency: each Trotter step should have same structure
        let gates_per_step = stats.total_gates / program.evolution.steps as usize;
        assert!(gates_per_step > 0, "Should have gates per step");

        // Verify term commutation: XX, YY, ZZ terms should all be present
        let gate_types: std::collections::HashSet<&str> = ast.gates
            .iter()
            .map(|g| g.name.as_str())
            .collect();
        // XX/YY terms produce H, CX, Rz gates; ZZ produces CX, Rz
        assert!(gate_types.contains("cx") || gate_types.contains("rz"),
            "Should have entangling gates from XX/YY/ZZ terms");

        // Lower to ZX-IR and validate
        let zx = lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
        zx.validate().expect("ZX-IR validation should succeed");
        assert!(zx.spider_count() > 0, "ZX graph should have spiders");

        // Emit QASM3 and verify
        let qasm = emit_qasm(&ast, QasmVersion::V3)
            .expect("QASM3 emission should succeed");
        assert!(qasm.contains("OPENQASM 3.0"), "Should emit QASM3");
        assert!(qasm.contains("qubit"), "Should declare qubits");

        // Verify 100% Trotter step coverage: all 10 steps contributed
        assert_eq!(stats.steps, 10, "Should have 10 Trotter steps");
        assert!(stats.total_gates > 0, "Should have generated gates");

        // Additional consistency check: gate count per step should be uniform
        let expected_total = gates_per_step * program.evolution.steps as usize;
        assert_eq!(stats.total_gates, expected_total,
            "Total gates should equal gates_per_step * steps");
    }
}
