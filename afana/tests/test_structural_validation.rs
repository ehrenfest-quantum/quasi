// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Structural validation tests on benchmark circuits.
//!
//! Builds realistic EhrenfestPrograms (Ising, Heisenberg, Rabi), trotterizes
//! them into ASTs, and verifies that `type_check_ast` accepts the result.
//! Also tests that hand-crafted invalid ASTs are correctly rejected.

use afana::ast::*;
use afana::cbor::*;
use afana::trotter::{trotterize, TrotterOrder};
use afana::type_check::type_check_ast;

// ── Benchmark program builders ──────────────────────────────────────────────

/// 4-qubit transverse-field Ising model: H = Σ J·ZᵢZᵢ₊₁ + Σ h·Xᵢ
fn ising_4q_program() -> EhrenfestProgram {
    let mut terms = Vec::new();
    // ZZ nearest-neighbour coupling
    for i in 0..3 {
        terms.push(PauliTerm {
            coefficient: 0.5,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
            ],
        });
    }
    // Transverse field X on each qubit
    for i in 0..4 {
        terms.push(PauliTerm {
            coefficient: 0.3,
            paulis: vec![PauliOpEntry { qubit: i, axis: PauliOp::X }],
        });
    }

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 4,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
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
            gate_fidelity_min: Some(0.99),
            readout_fidelity_min: None,
        },
    }
}

/// 3-qubit Heisenberg XXZ model: H = Σ (Jxy·XᵢXᵢ₊₁ + Jxy·YᵢYᵢ₊₁ + Jz·ZᵢZᵢ₊₁)
fn heisenberg_3q_program() -> EhrenfestProgram {
    let mut terms = Vec::new();
    for i in 0..2 {
        // XX coupling
        terms.push(PauliTerm {
            coefficient: 0.4,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::X },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::X },
            ],
        });
        // YY coupling
        terms.push(PauliTerm {
            coefficient: 0.4,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Y },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Y },
            ],
        });
        // ZZ coupling
        terms.push(PauliTerm {
            coefficient: 0.6,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
            ],
        });
    }

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 3,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: -1.2,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 10,
            dt_us: 0.1,
        },
        observables: vec![
            Observable::SZ { qubit: 0 },
            Observable::SZ { qubit: 1 },
            Observable::SZ { qubit: 2 },
        ],
        noise: NoiseConstraint {
            t1_us: 80.0,
            t2_us: 40.0,
            gate_fidelity_min: Some(0.995),
            readout_fidelity_min: Some(0.98),
        },
    }
}

/// Single-qubit Rabi oscillation: H = Ω·X
fn rabi_1q_program() -> EhrenfestProgram {
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
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
            }],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 0.5,
            steps: 5,
            dt_us: 0.1,
        },
        observables: vec![Observable::SX { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 50.0,
            t2_us: 30.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

// ── Valid benchmark circuit tests ───────────────────────────────────────────

#[test]
fn ising_4q_first_order_validates() {
    let program = ising_4q_program();
    let ast = trotterize(&program, TrotterOrder::First);
    assert_eq!(ast.n_qubits, 4);
    assert!(!ast.gates.is_empty());
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Ising 4q first-order should validate: {:?}", result.err());
}

#[test]
fn ising_4q_second_order_validates() {
    let program = ising_4q_program();
    let ast = trotterize(&program, TrotterOrder::Second);
    assert_eq!(ast.n_qubits, 4);
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Ising 4q second-order should validate: {:?}", result.err());
}

#[test]
fn heisenberg_3q_first_order_validates() {
    let program = heisenberg_3q_program();
    let ast = trotterize(&program, TrotterOrder::First);
    assert_eq!(ast.n_qubits, 3);
    assert!(!ast.gates.is_empty());
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Heisenberg 3q first-order should validate: {:?}", result.err());
}

#[test]
fn heisenberg_3q_second_order_validates() {
    let program = heisenberg_3q_program();
    let ast = trotterize(&program, TrotterOrder::Second);
    assert_eq!(ast.n_qubits, 3);
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Heisenberg 3q second-order should validate: {:?}", result.err());
}

#[test]
fn rabi_1q_first_order_validates() {
    let program = rabi_1q_program();
    let ast = trotterize(&program, TrotterOrder::First);
    assert_eq!(ast.n_qubits, 1);
    assert!(!ast.gates.is_empty());
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Rabi 1q first-order should validate: {:?}", result.err());
}

#[test]
fn rabi_1q_second_order_validates() {
    let program = rabi_1q_program();
    let ast = trotterize(&program, TrotterOrder::Second);
    assert_eq!(ast.n_qubits, 1);
    let result = type_check_ast(&ast);
    assert!(result.is_ok(), "Rabi 1q second-order should validate: {:?}", result.err());
}

// ── Gate count sanity checks ────────────────────────────────────────────────

#[test]
fn ising_4q_produces_substantial_circuit() {
    let program = ising_4q_program();
    let ast = trotterize(&program, TrotterOrder::First);
    // 7 Hamiltonian terms × 20 steps, each producing multiple gates
    assert!(
        ast.gates.len() > 100,
        "Ising 4q should produce many gates, got {}",
        ast.gates.len()
    );
    assert_eq!(ast.measures.len(), 4, "All 4 qubits should be measured");
}

#[test]
fn heisenberg_3q_produces_substantial_circuit() {
    let program = heisenberg_3q_program();
    let ast = trotterize(&program, TrotterOrder::First);
    // 6 Hamiltonian terms × 10 steps
    assert!(
        ast.gates.len() > 50,
        "Heisenberg 3q should produce many gates, got {}",
        ast.gates.len()
    );
    assert_eq!(ast.measures.len(), 3, "All 3 qubits should be measured");
}

#[test]
fn second_order_produces_more_gates_than_first() {
    let program = ising_4q_program();
    let ast_first = trotterize(&program, TrotterOrder::First);
    let ast_second = trotterize(&program, TrotterOrder::Second);
    assert!(
        ast_second.gates.len() > ast_first.gates.len(),
        "Second-order Trotter should produce more gates ({} vs {})",
        ast_second.gates.len(),
        ast_first.gates.len()
    );
}

// ── Invalid AST tests (hand-crafted) ────────────────────────────────────────

#[test]
fn qubit_out_of_range_rejected() {
    let ast = EhrenfestAst {
        name: "invalid_qubit".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate {
                name: GateName::H,
                qubits: vec![0],
                params: vec![],
            },
            Gate {
                name: GateName::Cx,
                qubits: vec![0, 5], // qubit 5 does not exist
                params: vec![],
            },
        ],
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should reject qubit index out of range");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("undefined qubit: q5")),
        "Should report undefined qubit q5, got: {:?}",
        errors
    );
}

#[test]
fn wrong_gate_arity_single_qubit_on_cx() {
    // CX requires 2 qubits but we only provide 1
    let ast = EhrenfestAst {
        name: "bad_arity".into(),
        n_qubits: 3,
        prepare: None,
        gates: Vec::new(),
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: vec![VariationalLoop {
            params: vec!["theta".into()],
            max_iter: 50,
            body: vec![VariationalGate {
                name: GateName::Cx,
                qubits: vec![0], // CX needs 2 qubits
                param_refs: vec![],
            }],
        }],
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should reject wrong gate arity");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("expects 2 qubit(s), got 1")),
        "Should report arity mismatch, got: {:?}",
        errors
    );
}

#[test]
fn wrong_gate_arity_three_qubits_on_h() {
    // H requires 1 qubit but we provide 3
    let ast = EhrenfestAst {
        name: "bad_arity_h".into(),
        n_qubits: 4,
        prepare: None,
        gates: Vec::new(),
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: vec![VariationalLoop {
            params: vec!["theta".into()],
            max_iter: 10,
            body: vec![VariationalGate {
                name: GateName::H,
                qubits: vec![0, 1, 2], // H needs 1 qubit
                param_refs: vec![],
            }],
        }],
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should reject H gate with 3 qubits");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("expects 1 qubit(s), got 3")),
        "Should report arity mismatch, got: {:?}",
        errors
    );
}

#[test]
fn measure_on_undefined_qubit_rejected() {
    let ast = EhrenfestAst {
        name: "bad_measure".into(),
        n_qubits: 2,
        prepare: None,
        gates: Vec::new(),
        measures: vec![Measure { qubit: 10, cbit: 0 }], // qubit 10 does not exist
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should reject measure on undefined qubit");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("undefined qubit: q10")),
        "Should report undefined qubit q10, got: {:?}",
        errors
    );
}

#[test]
fn parametric_gate_without_params_rejected() {
    let ast = EhrenfestAst {
        name: "no_param_rz".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![Gate {
            name: GateName::Rz,
            qubits: vec![0],
            params: vec![], // Rz requires a parameter
        }],
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    let result = type_check_ast(&ast);
    assert!(result.is_err(), "Should reject parametric gate without params");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("missing parameters")),
        "Should report missing parameters, got: {:?}",
        errors
    );
}

// ── Trotterized output always valid (property-style) ────────────────────────

#[test]
fn all_benchmarks_trotterize_to_valid_asts() {
    let benchmarks: Vec<(&str, EhrenfestProgram)> = vec![
        ("ising_4q", ising_4q_program()),
        ("heisenberg_3q", heisenberg_3q_program()),
        ("rabi_1q", rabi_1q_program()),
    ];

    for (name, program) in &benchmarks {
        for order in &[TrotterOrder::First, TrotterOrder::Second] {
            let ast = trotterize(program, *order);
            let result = type_check_ast(&ast);
            assert!(
                result.is_ok(),
                "Benchmark '{}' with {:?} order should produce a valid AST: {:?}",
                name,
                order,
                result.err()
            );

            // Every gate should reference only valid qubits
            for gate in &ast.gates {
                for &q in &gate.qubits {
                    assert!(
                        q < ast.n_qubits,
                        "Benchmark '{}': gate {} references qubit {} but n_qubits={}",
                        name,
                        gate.name,
                        q,
                        ast.n_qubits
                    );
                }
            }

            // Every measure should reference a valid qubit
            for m in &ast.measures {
                assert!(
                    m.qubit < ast.n_qubits,
                    "Benchmark '{}': measure references qubit {} but n_qubits={}",
                    name,
                    m.qubit,
                    ast.n_qubits
                );
            }

            // All parametric gates should have parameters
            for gate in &ast.gates {
                if gate.name.is_parametric() {
                    assert!(
                        !gate.params.is_empty(),
                        "Benchmark '{}': parametric gate {} has no parameters",
                        name,
                        gate.name
                    );
                }
            }

            // Gate arity should match qubit count
            for gate in &ast.gates {
                assert_eq!(
                    gate.qubits.len(),
                    gate.name.arity(),
                    "Benchmark '{}': gate {} has {} qubits but arity is {}",
                    name,
                    gate.name,
                    gate.qubits.len(),
                    gate.name.arity()
                );
            }
        }
    }
}
