// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: full Hamiltonian → Trotterization → QASM pipeline.
//!
//! Tests the enhanced pipeline end-to-end: validation, Trotterization with
//! statistics, noise-budget checking, type checking, ZX-IR lowering, and
//! QASM emission.

use afana::cbor::*;
use afana::emit::{self, QasmVersion};
use afana::lower;
use afana::noise;
use afana::observable;
use afana::trotter::{self, TrotterOrder};
use afana::type_check;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn rabi_program() -> EhrenfestProgram {
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 1,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 1.0,
                paulis: vec![PauliOpEntry {
                    qubit: 0,
                    axis: PauliOp::X,
                }],
            }],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 10,
            dt_us: 0.1,
        },
        observables: vec![Observable::SX { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: Some(0.90),
            readout_fidelity_min: None,
        },
    }
}

fn ising_program() -> EhrenfestProgram {
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![
                PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![
                        PauliOpEntry {
                            qubit: 0,
                            axis: PauliOp::Z,
                        },
                        PauliOpEntry {
                            qubit: 1,
                            axis: PauliOp::Z,
                        },
                    ],
                },
                PauliTerm {
                    coefficient: 0.3,
                    paulis: vec![PauliOpEntry {
                        qubit: 0,
                        axis: PauliOp::X,
                    }],
                },
                PauliTerm {
                    coefficient: 0.3,
                    paulis: vec![PauliOpEntry {
                        qubit: 1,
                        axis: PauliOp::X,
                    }],
                },
            ],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 2.0,
            steps: 20,
            dt_us: 0.1,
        },
        observables: vec![Observable::SZ { qubit: 0 }, Observable::E],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: Some(0.90),
            readout_fidelity_min: None,
        },
    }
}

fn heisenberg_program() -> EhrenfestProgram {
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 3,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![
                // XX terms
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::X },
                        PauliOpEntry { qubit: 1, axis: PauliOp::X },
                    ],
                },
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 1, axis: PauliOp::X },
                        PauliOpEntry { qubit: 2, axis: PauliOp::X },
                    ],
                },
                // YY terms
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::Y },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Y },
                    ],
                },
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 1, axis: PauliOp::Y },
                        PauliOpEntry { qubit: 2, axis: PauliOp::Y },
                    ],
                },
                // ZZ terms
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Z },
                    ],
                },
                PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 1, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 2, axis: PauliOp::Z },
                    ],
                },
            ],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 10,
            dt_us: 0.1,
        },
        observables: vec![Observable::E],
        noise: NoiseConstraint {
            t1_us: 200.0,
            t2_us: 100.0,
            gate_fidelity_min: Some(0.80),
            readout_fidelity_min: None,
        },
    }
}

/// Run the full pipeline: validate → trotterize → type check → lower → noise → emit.
fn full_pipeline(program: &EhrenfestProgram, order: TrotterOrder) -> String {
    trotter::validate_hamiltonian(program).expect("validation should pass");

    let (mut ast, stats) =
        trotter::trotterize_with_stats(program, order).expect("trotterize should succeed");

    let _budget = trotter::check_noise_budget(&stats, &program.noise);

    type_check::type_check_ast(&ast).expect("type check should pass");

    let zx = lower::lower_ast_to_zx(&ast).expect("ZX-IR lowering should succeed");
    zx.validate().expect("ZX-IR should be valid");

    let _noise_report = noise::analyze_noise(&ast, &program.noise, &program.evolution);

    let measurements =
        observable::synthesize_measurements(&program.observables, program.system.n_qubits);
    observable::apply_measurement_circuits(&mut ast, &measurements);

    emit::emit_qasm(&ast, QasmVersion::V3).expect("QASM emission should succeed")
}

// ── End-to-end pipeline tests ───────────────────────────────────────────────

#[test]
fn pipeline_rabi_first_order() {
    let qasm = full_pipeline(&rabi_program(), TrotterOrder::First);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("qubit[1] q;"));
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("rz("));
}

#[test]
fn pipeline_rabi_second_order() {
    let qasm = full_pipeline(&rabi_program(), TrotterOrder::Second);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("h q[0];"));
}

#[test]
fn pipeline_rabi_fourth_order() {
    let qasm = full_pipeline(&rabi_program(), TrotterOrder::Fourth);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("h q[0];"));
    // Fourth-order should produce more gates than first-order.
    let gate_lines: usize = qasm
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty()
                && !l.starts_with("OPENQASM")
                && !l.starts_with("include")
                && !l.starts_with("qubit")
                && !l.starts_with("bit")
                && !l.starts_with("//")
                && !l.contains("measure")
        })
        .count();
    assert!(gate_lines > 30, "4th-order Rabi should produce many gates, got {gate_lines}");
}

#[test]
fn pipeline_ising_all_orders() {
    for order in [TrotterOrder::First, TrotterOrder::Second, TrotterOrder::Fourth] {
        let qasm = full_pipeline(&ising_program(), order);
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("qubit[2] q;"));
        // ZZ term requires CNOT ladder.
        assert!(qasm.contains("cx q[0], q[1];"));
        // Both qubits measured.
        assert!(qasm.contains("measure q[0]"));
        assert!(qasm.contains("measure q[1]"));
    }
}

#[test]
fn pipeline_heisenberg_produces_valid_qasm() {
    let qasm = full_pipeline(&heisenberg_program(), TrotterOrder::Second);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("qubit[3] q;"));
    // Heisenberg model includes XX, YY, ZZ terms.
    // XX/YY terms need H and Sdg basis changes; ZZ needs CNOT ladder.
    assert!(qasm.contains("cx "));
    assert!(qasm.contains("rz("));
    // All 3 qubits measured.
    assert!(qasm.contains("measure q[2]"));
}

// ── Statistics and error bound tests ────────────────────────────────────────

#[test]
fn pipeline_stats_consistent_across_orders() {
    let program = ising_program();
    let (_, s1) = trotter::trotterize_with_stats(&program, TrotterOrder::First).unwrap();
    let (_, s2) = trotter::trotterize_with_stats(&program, TrotterOrder::Second).unwrap();
    let (_, s4) = trotter::trotterize_with_stats(&program, TrotterOrder::Fourth).unwrap();

    // All should report the same Hamiltonian metadata.
    assert_eq!(s1.hamiltonian_terms, s2.hamiltonian_terms);
    assert_eq!(s2.hamiltonian_terms, s4.hamiltonian_terms);
    assert_eq!(s1.steps, s2.steps);

    // Gate counts should increase with order.
    assert!(s2.total_gates > s1.total_gates);
    assert!(s4.total_gates > s2.total_gates);

    // Error bounds should decrease with order.
    assert!(s2.error_bound < s1.error_bound);
    assert!(s4.error_bound < s2.error_bound);
}

#[test]
fn pipeline_error_bound_decreases_with_more_steps() {
    let mut program = ising_program();
    let e_20 = trotter::estimate_trotter_error(&program, TrotterOrder::First);

    program.evolution.steps = 40;
    program.evolution.dt_us = program.evolution.total_us / 40.0;
    let e_40 = trotter::estimate_trotter_error(&program, TrotterOrder::First);

    assert!(
        e_40 < e_20,
        "doubling steps should reduce error: {e_40:.2e} vs {e_20:.2e}"
    );
}

// ── Noise budget integration tests ──────────────────────────────────────────

#[test]
fn pipeline_noise_budget_passes_for_small_circuit() {
    let program = rabi_program();
    let (_, stats) = trotter::trotterize_with_stats(&program, TrotterOrder::First).unwrap();
    let budget = trotter::check_noise_budget(&stats, &program.noise);
    assert!(budget.within_t1, "small Rabi circuit should fit T1");
    assert!(budget.within_t2, "small Rabi circuit should fit T2");
    assert!(budget.fidelity_ok, "small circuit fidelity should be OK");
    assert!(budget.warnings.is_empty());
}

#[test]
fn pipeline_noise_budget_warns_for_deep_circuit() {
    let program = heisenberg_program();
    let (_, stats) = trotter::trotterize_with_stats(&program, TrotterOrder::Fourth).unwrap();
    // Fourth-order Heisenberg produces a deep circuit.
    let tight_noise = NoiseConstraint {
        t1_us: 1.0, // Very short T1
        t2_us: 0.5, // Very short T2
        gate_fidelity_min: Some(0.9999),
        readout_fidelity_min: None,
    };
    let budget = trotter::check_noise_budget(&stats, &tight_noise);
    assert!(!budget.within_t1);
    assert!(!budget.within_t2);
    assert!(!budget.fidelity_ok);
    assert!(budget.warnings.len() >= 2, "should have multiple warnings");
}

// ── Validation rejection tests ──────────────────────────────────────────────

#[test]
fn pipeline_rejects_empty_hamiltonian() {
    let mut program = rabi_program();
    program.hamiltonian.terms.clear();
    assert!(trotter::validate_hamiltonian(&program).is_err());
    assert!(trotter::trotterize_with_stats(&program, TrotterOrder::First).is_err());
}

#[test]
fn pipeline_rejects_qubit_out_of_range() {
    let mut program = rabi_program();
    program.hamiltonian.terms[0].paulis[0].qubit = 10;
    assert!(trotter::validate_hamiltonian(&program).is_err());
}

#[test]
fn pipeline_rejects_nan_coefficient() {
    let mut program = rabi_program();
    program.hamiltonian.terms[0].coefficient = f64::NAN;
    assert!(trotter::validate_hamiltonian(&program).is_err());
}

// ── Fourth-order correctness tests ──────────────────────────────────────────

#[test]
fn fourth_order_trotter_produces_5x_second_order_gates() {
    let program = ising_program();
    let ast2 = trotter::trotterize(&program, TrotterOrder::Second);
    let ast4 = trotter::trotterize(&program, TrotterOrder::Fourth);
    // S4 = 5 × S2 sub-steps, so gate ratio should be exactly 5.
    let ratio = ast4.gates.len() as f64 / ast2.gates.len() as f64;
    assert!(
        (ratio - 5.0).abs() < 1e-10,
        "4th/2nd gate ratio should be 5.0, got {ratio}",
    );
}

#[test]
fn fourth_order_heisenberg_type_checks_and_lowers() {
    let program = heisenberg_program();
    let ast = trotter::trotterize(&program, TrotterOrder::Fourth);
    type_check::type_check_ast(&ast).expect("4th-order Heisenberg should type check");
    let zx = lower::lower_ast_to_zx(&ast).expect("should lower to ZX-IR");
    zx.validate().expect("ZX-IR should validate");
}

// ── CBOR roundtrip through pipeline ─────────────────────────────────────────

#[test]
fn cbor_roundtrip_then_pipeline() {
    let program = ising_program();

    // Serialize to CBOR.
    let mut buf = Vec::new();
    ciborium::into_writer(&program, &mut buf).unwrap();

    // Deserialize back.
    let decoded = afana::cbor::from_cbor(&buf).unwrap();

    // Run full pipeline on deserialized program.
    let qasm = full_pipeline(&decoded, TrotterOrder::Second);
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("cx q[0], q[1];"));
}
