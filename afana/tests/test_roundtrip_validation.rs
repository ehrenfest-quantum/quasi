// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: round-trip validation for ZX-IR to QASM3.
//!
//! Build Bell and GHZ circuit ASTs directly, emit QASM 3.0, then verify
//! the output contains the correct header, qubit declarations, gate
//! sequence, and measurements.

use afana::ast::*;
use afana::emit::{emit_qasm, QasmVersion};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a 2-qubit Bell state circuit: H q[0]; CX q[0], q[1]; measure both.
fn bell_circuit() -> EhrenfestAst {
    EhrenfestAst {
        name: "bell".into(),
        n_qubits: 2,
        prepare: None,
        gates: vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
        measures: vec![
            Measure { qubit: 0, cbit: 0 },
            Measure { qubit: 1, cbit: 1 },
        ],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

/// Build an N-qubit GHZ circuit: H q[0]; CX q[0],q[1]; CX q[1],q[2]; ...
fn ghz_circuit(n: usize) -> EhrenfestAst {
    assert!(n >= 2, "GHZ requires at least 2 qubits");
    let mut gates = vec![Gate { name: GateName::H, qubits: vec![0], params: vec![] }];
    for i in 0..n - 1 {
        gates.push(Gate { name: GateName::Cx, qubits: vec![i, i + 1], params: vec![] });
    }
    let measures = (0..n).map(|i| Measure { qubit: i, cbit: i }).collect();
    EhrenfestAst {
        name: format!("ghz_{n}"),
        n_qubits: n,
        prepare: None,
        gates,
        measures,
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

// ── Bell state tests ────────────────────────────────────────────────────────

#[test]
fn bell_qasm3_header() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    assert!(qasm.starts_with("OPENQASM 3.0;"), "must start with QASM 3.0 header");
    assert!(qasm.contains("include \"stdgates.inc\";"), "must include stdgates");
}

#[test]
fn bell_qasm3_qubit_declaration() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    assert!(qasm.contains("qubit[2] q;"), "must declare 2 qubits");
    assert!(qasm.contains("bit[2] c;"), "must declare 2 classical bits");
}

#[test]
fn bell_qasm3_gate_sequence() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    let lines: Vec<&str> = qasm.lines().collect();

    // Find the gate lines and verify their order.
    let h_pos = lines.iter().position(|l| l.trim() == "h q[0];");
    let cx_pos = lines.iter().position(|l| l.trim() == "cx q[0], q[1];");

    assert!(h_pos.is_some(), "must contain H gate on q[0]");
    assert!(cx_pos.is_some(), "must contain CX gate on q[0], q[1]");
    assert!(h_pos.unwrap() < cx_pos.unwrap(), "H must come before CX");
}

#[test]
fn bell_qasm3_measurements() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    assert!(qasm.contains("c[0] = measure q[0];"), "must measure q[0] into c[0]");
    assert!(qasm.contains("c[1] = measure q[1];"), "must measure q[1] into c[1]");
}

#[test]
fn bell_qasm3_gate_count() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    // Exactly 2 gate lines: h and cx.
    let gate_lines: Vec<&str> = qasm
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("h ") || t.starts_with("cx ")
        })
        .collect();
    assert_eq!(gate_lines.len(), 2, "Bell circuit must have exactly 2 gates");
}

// ── GHZ state tests (3 qubits) ─────────────────────────────────────────────

#[test]
fn ghz3_qasm3_header() {
    let qasm = emit_qasm(&ghz_circuit(3), QasmVersion::V3).unwrap();
    assert!(qasm.starts_with("OPENQASM 3.0;"));
    assert!(qasm.contains("include \"stdgates.inc\";"));
}

#[test]
fn ghz3_qasm3_qubit_declaration() {
    let qasm = emit_qasm(&ghz_circuit(3), QasmVersion::V3).unwrap();
    assert!(qasm.contains("qubit[3] q;"), "must declare 3 qubits");
    assert!(qasm.contains("bit[3] c;"), "must declare 3 classical bits");
}

#[test]
fn ghz3_qasm3_gate_sequence() {
    let qasm = emit_qasm(&ghz_circuit(3), QasmVersion::V3).unwrap();
    let lines: Vec<&str> = qasm.lines().collect();

    let h_pos = lines.iter().position(|l| l.trim() == "h q[0];");
    let cx01_pos = lines.iter().position(|l| l.trim() == "cx q[0], q[1];");
    let cx12_pos = lines.iter().position(|l| l.trim() == "cx q[1], q[2];");

    assert!(h_pos.is_some(), "must contain H gate on q[0]");
    assert!(cx01_pos.is_some(), "must contain CX q[0], q[1]");
    assert!(cx12_pos.is_some(), "must contain CX q[1], q[2]");

    // Verify ordering: H < CX(0,1) < CX(1,2).
    assert!(h_pos.unwrap() < cx01_pos.unwrap(), "H must come before first CX");
    assert!(cx01_pos.unwrap() < cx12_pos.unwrap(), "CX chain must be in order");
}

#[test]
fn ghz3_qasm3_measurements() {
    let qasm = emit_qasm(&ghz_circuit(3), QasmVersion::V3).unwrap();
    for i in 0..3 {
        let expected = format!("c[{i}] = measure q[{i}];");
        assert!(qasm.contains(&expected), "must measure q[{i}] into c[{i}]");
    }
}

#[test]
fn ghz3_qasm3_gate_count() {
    let qasm = emit_qasm(&ghz_circuit(3), QasmVersion::V3).unwrap();
    // 1 H + 2 CX = 3 gates total.
    let gate_lines: Vec<&str> = qasm
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("h ") || t.starts_with("cx ")
        })
        .collect();
    assert_eq!(gate_lines.len(), 3, "GHZ-3 must have exactly 3 gates (1 H + 2 CX)");
}

// ── GHZ state tests (5 qubits) ─────────────────────────────────────────────

#[test]
fn ghz5_qasm3_full_validation() {
    let qasm = emit_qasm(&ghz_circuit(5), QasmVersion::V3).unwrap();

    // Header.
    assert!(qasm.starts_with("OPENQASM 3.0;"));
    assert!(qasm.contains("include \"stdgates.inc\";"));

    // Declarations.
    assert!(qasm.contains("qubit[5] q;"));
    assert!(qasm.contains("bit[5] c;"));

    // Gate sequence: 1 H + 4 CX = 5 gates.
    assert!(qasm.contains("h q[0];"));
    for i in 0..4 {
        let cx = format!("cx q[{i}], q[{}];", i + 1);
        assert!(qasm.contains(&cx), "must contain {cx}");
    }

    // All 5 qubits measured.
    for i in 0..5 {
        let m = format!("c[{i}] = measure q[{i}];");
        assert!(qasm.contains(&m), "must contain {m}");
    }

    // Gate count.
    let gate_count = qasm
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("h ") || t.starts_with("cx ")
        })
        .count();
    assert_eq!(gate_count, 5, "GHZ-5 must have exactly 5 gates (1 H + 4 CX)");
}

// ── Structural validation ───────────────────────────────────────────────────

#[test]
fn bell_qasm3_section_ordering() {
    let qasm = emit_qasm(&bell_circuit(), QasmVersion::V3).unwrap();
    let lines: Vec<&str> = qasm.lines().collect();

    // Find the positions of key sections.
    let header_pos = lines.iter().position(|l| l.starts_with("OPENQASM")).unwrap();
    let qubit_pos = lines.iter().position(|l| l.starts_with("qubit")).unwrap();
    let first_gate = lines.iter().position(|l| l.trim() == "h q[0];").unwrap();
    let first_meas = lines.iter().position(|l| l.contains("measure")).unwrap();

    // Verify ordering: header < declarations < gates < measurements.
    assert!(header_pos < qubit_pos, "header before qubit decl");
    assert!(qubit_pos < first_gate, "qubit decl before gates");
    assert!(first_gate < first_meas, "gates before measurements");
}
