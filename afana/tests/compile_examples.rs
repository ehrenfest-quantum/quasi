// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: full pipeline — parse → trotterize → emit QASM3.

use std::path::PathBuf;

use afana::cbor;
use afana::emit::{self, QasmVersion};
use afana::trotter::{self, TrotterOrder};

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("{name}.paul"))
}

fn compile_to_qasm3(name: &str) -> String {
    let prog = cbor::from_cbor_file(&example_path(name)).unwrap();
    let ast = trotter::trotterize(&prog, TrotterOrder::First);
    emit::emit_qasm(&ast, QasmVersion::V3).unwrap()
}

#[test]
fn compile_rabi_produces_qasm3() {
    let qasm = compile_to_qasm3("rabi");
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("include \"stdgates.inc\";"));
    assert!(qasm.contains("qubit[1] q;"));
    // Rabi: X rotation → H, Rz, H pattern
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("rz("));
    // Measurement
    assert!(qasm.contains("measure q[0]"));
}

#[test]
fn compile_ising_produces_qasm3() {
    let qasm = compile_to_qasm3("ising");
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("qubit[2] q;"));
    // ZZ term needs CNOT ladder
    assert!(qasm.contains("cx q[0], q[1];"));
    // Both qubits measured
    assert!(qasm.contains("measure q[0]"));
    assert!(qasm.contains("measure q[1]"));
}

#[test]
fn compile_heisenberg_produces_qasm3() {
    let qasm = compile_to_qasm3("heisenberg");
    assert!(qasm.contains("OPENQASM 3.0;"));
    assert!(qasm.contains("qubit[4] q;"));
    // 4-qubit system → all measured
    assert!(qasm.contains("measure q[3]"));
    // Should contain many gates (9 terms × 20 steps)
    let gate_lines = qasm.lines().filter(|l| {
        let l = l.trim();
        !l.is_empty()
            && !l.starts_with("OPENQASM")
            && !l.starts_with("include")
            && !l.starts_with("qubit")
            && !l.starts_with("bit")
            && !l.starts_with("//")
            && !l.contains("measure")
    }).count();
    assert!(gate_lines > 100, "Heisenberg 4q should produce many gates, got {gate_lines}");
}

#[test]
fn compile_rabi_second_order() {
    let prog = cbor::from_cbor_file(&example_path("rabi")).unwrap();
    let ast = trotter::trotterize(&prog, TrotterOrder::Second);
    let qasm = emit::emit_qasm(&ast, QasmVersion::V3).unwrap();
    assert!(qasm.contains("OPENQASM 3.0;"));
    // Second-order doubles the gates per step (forward + backward half-steps).
    assert!(qasm.contains("h q[0];"));
}
