// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: parse .paul binary files and validate field values.

use std::path::PathBuf;

use afana::cbor::{self, Observable, PauliOp};

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("{name}.paul"))
}

#[test]
fn parse_rabi() {
    let prog = cbor::from_cbor_file(&example_path("rabi")).unwrap();
    assert_eq!(prog.version, 1);
    assert_eq!(prog.system.n_qubits, 1);
    assert_eq!(prog.hamiltonian.terms.len(), 1);
    // Single X term with coefficient ≈ -0.015708
    let term = &prog.hamiltonian.terms[0];
    assert!((term.coefficient - (-0.015708)).abs() < 1e-6);
    assert_eq!(term.paulis.len(), 1);
    assert_eq!(term.paulis[0].qubit, 0);
    assert_eq!(term.paulis[0].axis, PauliOp::X);
    assert_eq!(prog.hamiltonian.constant_offset, 0.0);
    // Evolution: 0.1 μs, 5 steps
    assert!((prog.evolution.total_us - 0.1).abs() < 1e-10);
    assert_eq!(prog.evolution.steps, 5);
    assert!((prog.evolution.dt_us - 0.02).abs() < 1e-10);
    // Observable: SX on qubit 0
    assert_eq!(prog.observables.len(), 1);
    assert!(matches!(&prog.observables[0], Observable::SX { qubit: 0 }));
    // Noise
    assert_eq!(prog.noise.t1_us, 50.0);
    assert_eq!(prog.noise.t2_us, 30.0);
}

#[test]
fn parse_ising() {
    let prog = cbor::from_cbor_file(&example_path("ising")).unwrap();
    assert_eq!(prog.version, 1);
    assert_eq!(prog.system.n_qubits, 2);
    assert_eq!(prog.hamiltonian.terms.len(), 3);
    // First term: ZZ coupling
    assert_eq!(prog.hamiltonian.terms[0].paulis.len(), 2);
    assert_eq!(prog.hamiltonian.terms[0].paulis[0].axis, PauliOp::Z);
    assert_eq!(prog.hamiltonian.terms[0].paulis[1].axis, PauliOp::Z);
    // Backend hint
    assert_eq!(prog.system.backend_hint.as_deref(), Some("ibm_torino"));
    // Two SZ observables
    assert_eq!(prog.observables.len(), 2);
    assert!(matches!(&prog.observables[0], Observable::SZ { qubit: 0 }));
    assert!(matches!(&prog.observables[1], Observable::SZ { qubit: 1 }));
    // Noise with gate fidelity
    assert_eq!(prog.noise.t1_us, 100.0);
    assert_eq!(prog.noise.t2_us, 80.0);
    assert!((prog.noise.gate_fidelity_min.unwrap() - 0.999).abs() < 1e-10);
}

#[test]
fn parse_heisenberg() {
    let prog = cbor::from_cbor_file(&example_path("heisenberg")).unwrap();
    assert_eq!(prog.version, 1);
    assert_eq!(prog.system.n_qubits, 4);
    assert_eq!(prog.hamiltonian.terms.len(), 9); // 3 pairs × 3 Pauli types
    // Energy observable
    assert_eq!(prog.observables.len(), 1);
    assert!(matches!(&prog.observables[0], Observable::E));
    // Readout fidelity
    assert!((prog.noise.readout_fidelity_min.unwrap() - 0.995).abs() < 1e-10);
    // Evolution: 2 μs, 20 steps
    assert_eq!(prog.evolution.steps, 20);
    assert!((prog.evolution.total_us - 2.0).abs() < 1e-10);
}
