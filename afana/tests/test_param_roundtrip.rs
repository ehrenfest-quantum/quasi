// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: parameter binding round-trip validation.
//!
//! Verifies that circuits with variational loops and parameter substitution
//! produce valid QASM with concrete angles, and that multi-controlled gates
//! (CCX) emit correctly alongside substituted variational gates.

use std::collections::HashMap;

use afana::ast::*;
use afana::emit::{emit_qasm, substitute_params, QasmVersion};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a 2-qubit variational ansatz: Ry(theta) q[0]; Rz(phi) q[1]; CX q[0],q[1].
fn variational_ansatz() -> EhrenfestAst {
    EhrenfestAst {
        name: "vqe_ansatz".into(),
        n_qubits: 2,
        prepare: None,
        gates: Vec::new(),
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: vec![VariationalLoop {
            params: vec!["theta".into(), "phi".into()],
            max_iter: 100,
            body: vec![
                VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["theta".into()],
                },
                VariationalGate {
                    name: GateName::Rz,
                    qubits: vec![1],
                    param_refs: vec!["phi".into()],
                },
                VariationalGate {
                    name: GateName::Cx,
                    qubits: vec![0, 1],
                    param_refs: vec![],
                },
            ],
        }],
    }
}

/// Build a 3-qubit Toffoli (CCX) circuit.
fn ccx_circuit() -> EhrenfestAst {
    EhrenfestAst {
        name: "ccx_test".into(),
        n_qubits: 3,
        prepare: None,
        gates: vec![
            Gate { name: GateName::X, qubits: vec![0], params: vec![] },
            Gate { name: GateName::X, qubits: vec![1], params: vec![] },
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
        ],
        measures: vec![
            Measure { qubit: 2, cbit: 0 },
        ],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

/// Build a mixed circuit: fixed gates (H, CCX) + variational loop (Ry, Rz, CX).
fn mixed_circuit() -> EhrenfestAst {
    EhrenfestAst {
        name: "mixed".into(),
        n_qubits: 3,
        prepare: None,
        gates: vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
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
        variational_loops: vec![VariationalLoop {
            params: vec!["alpha".into(), "beta".into()],
            max_iter: 50,
            body: vec![
                VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["alpha".into()],
                },
                VariationalGate {
                    name: GateName::Rz,
                    qubits: vec![1],
                    param_refs: vec!["beta".into()],
                },
            ],
        }],
    }
}

// ── Variational substitution tests ──────────────────────────────────────────

#[test]
fn variational_substitute_emits_concrete_angles() {
    let ast = variational_ansatz();
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), std::f64::consts::FRAC_PI_4);
    bindings.insert("phi".to_string(), std::f64::consts::FRAC_PI_2);

    let resolved = substitute_params(&ast, &bindings).unwrap();

    // Variational loops should be fully consumed.
    assert!(
        resolved.variational_loops.is_empty(),
        "all loops must be resolved after full substitution"
    );

    // Emit QASM 3.0 and verify concrete angles appear.
    let qasm = emit_qasm(&resolved, QasmVersion::V3).unwrap();

    assert!(
        qasm.contains("ry(pi/4) q[0];"),
        "expected ry(pi/4), got:\n{qasm}"
    );
    assert!(
        qasm.contains("rz(pi/2) q[1];"),
        "expected rz(pi/2), got:\n{qasm}"
    );
    assert!(
        qasm.contains("cx q[0], q[1];"),
        "CX entangler must appear in output:\n{qasm}"
    );

    // No symbolic parameter names should remain.
    assert!(
        !qasm.contains("theta"),
        "symbolic name 'theta' must not appear in resolved QASM"
    );
    assert!(
        !qasm.contains("phi"),
        "symbolic name 'phi' must not appear in resolved QASM"
    );
}

#[test]
fn variational_substitute_removes_loop_metadata() {
    let ast = variational_ansatz();
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), 0.0);
    bindings.insert("phi".to_string(), std::f64::consts::PI);

    let resolved = substitute_params(&ast, &bindings).unwrap();
    let qasm = emit_qasm(&resolved, QasmVersion::V3).unwrap();

    // After substitution, no variational loop comments should appear.
    assert!(
        !qasm.contains("Variational ansatz"),
        "variational loop metadata must not appear after full substitution"
    );
    assert!(
        !qasm.contains("mutable float"),
        "parameter declarations must not appear after full substitution"
    );
}

// ── CCX (Toffoli) gate tests ────────────────────────────────────────────────

#[test]
fn ccx_circuit_emits_toffoli_gate() {
    let qasm = emit_qasm(&ccx_circuit(), QasmVersion::V3).unwrap();

    assert!(
        qasm.contains("ccx q[0], q[1], q[2];"),
        "must contain ccx gate: {qasm}"
    );
    assert!(
        qasm.contains("qubit[3] q;"),
        "must declare 3 qubits for CCX circuit"
    );
}

#[test]
fn ccx_circuit_emits_correct_gate_order() {
    let qasm = emit_qasm(&ccx_circuit(), QasmVersion::V3).unwrap();
    let lines: Vec<&str> = qasm.lines().collect();

    let x0_pos = lines.iter().position(|l| l.trim() == "x q[0];");
    let x1_pos = lines.iter().position(|l| l.trim() == "x q[1];");
    let ccx_pos = lines.iter().position(|l| l.trim() == "ccx q[0], q[1], q[2];");

    assert!(x0_pos.is_some(), "must contain X q[0]");
    assert!(x1_pos.is_some(), "must contain X q[1]");
    assert!(ccx_pos.is_some(), "must contain CCX");

    assert!(
        x0_pos.unwrap() < ccx_pos.unwrap(),
        "X gates must precede CCX"
    );
    assert!(
        x1_pos.unwrap() < ccx_pos.unwrap(),
        "X gates must precede CCX"
    );
}

#[test]
fn ccx_circuit_qasm2() {
    let qasm = emit_qasm(&ccx_circuit(), QasmVersion::V2).unwrap();

    assert!(qasm.contains("OPENQASM 2.0;"));
    assert!(qasm.contains("ccx q[0], q[1], q[2];"));
    assert!(qasm.contains("measure q[2] -> c[0];"));
}

// ── Mixed circuit: fixed gates + substituted variational gates ──────────────

#[test]
fn mixed_circuit_full_roundtrip() {
    let ast = mixed_circuit();
    let mut bindings = HashMap::new();
    bindings.insert("alpha".to_string(), std::f64::consts::FRAC_PI_4);
    bindings.insert("beta".to_string(), 3.0 * std::f64::consts::FRAC_PI_4);

    let resolved = substitute_params(&ast, &bindings).unwrap();

    // Verify structure: 3 fixed gates + 2 resolved = 5 total.
    assert_eq!(
        resolved.gates.len(),
        5,
        "expected 3 fixed + 2 resolved gates"
    );
    assert!(
        resolved.variational_loops.is_empty(),
        "all loops must be resolved"
    );

    let qasm = emit_qasm(&resolved, QasmVersion::V3).unwrap();

    // Fixed gates present.
    assert!(qasm.contains("h q[0];"), "H on q[0] must appear");
    assert!(qasm.contains("h q[1];"), "H on q[1] must appear");
    assert!(
        qasm.contains("ccx q[0], q[1], q[2];"),
        "CCX must appear"
    );

    // Substituted variational gates with concrete angles.
    assert!(
        qasm.contains("ry(pi/4) q[0];"),
        "expected ry(pi/4), got:\n{qasm}"
    );
    assert!(
        qasm.contains("rz(3*pi/4) q[1];"),
        "expected rz(3*pi/4), got:\n{qasm}"
    );

    // No symbolic names.
    assert!(
        !qasm.contains("alpha"),
        "symbolic name 'alpha' must not appear"
    );
    assert!(
        !qasm.contains("beta"),
        "symbolic name 'beta' must not appear"
    );

    // Measurements present.
    for i in 0..3 {
        let m = format!("c[{i}] = measure q[{i}];");
        assert!(qasm.contains(&m), "must contain {m}");
    }
}

#[test]
fn mixed_circuit_gate_ordering_preserved() {
    let ast = mixed_circuit();
    let mut bindings = HashMap::new();
    bindings.insert("alpha".to_string(), 1.0);
    bindings.insert("beta".to_string(), 2.0);

    let resolved = substitute_params(&ast, &bindings).unwrap();
    let qasm = emit_qasm(&resolved, QasmVersion::V3).unwrap();
    let lines: Vec<&str> = qasm.lines().collect();

    // Fixed gates come first (H, H, CCX), then resolved gates (Ry, Rz).
    let h0_pos = lines.iter().position(|l| l.trim() == "h q[0];").unwrap();
    let ccx_pos = lines
        .iter()
        .position(|l| l.trim() == "ccx q[0], q[1], q[2];")
        .unwrap();
    let ry_pos = lines
        .iter()
        .position(|l| l.trim().starts_with("ry("))
        .unwrap();
    let rz_pos = lines
        .iter()
        .position(|l| l.trim().starts_with("rz("))
        .unwrap();

    assert!(
        h0_pos < ccx_pos,
        "fixed H must come before fixed CCX"
    );
    assert!(
        ccx_pos < ry_pos,
        "fixed gates must come before resolved variational gates"
    );
    assert!(
        ry_pos < rz_pos,
        "resolved gates must preserve body order"
    );
}

// ── QASM version consistency ────────────────────────────────────────────────

#[test]
fn substituted_circuit_valid_qasm2_and_qasm3() {
    let ast = variational_ansatz();
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), std::f64::consts::FRAC_PI_2);
    bindings.insert("phi".to_string(), std::f64::consts::FRAC_PI_4);

    let resolved = substitute_params(&ast, &bindings).unwrap();

    let qasm2 = emit_qasm(&resolved, QasmVersion::V2).unwrap();
    let qasm3 = emit_qasm(&resolved, QasmVersion::V3).unwrap();

    // Both versions must contain the concrete gates.
    for qasm in [&qasm2, &qasm3] {
        assert!(qasm.contains("ry(pi/2) q[0];"), "ry(pi/2) in output");
        assert!(qasm.contains("rz(pi/4) q[1];"), "rz(pi/4) in output");
        assert!(qasm.contains("cx q[0], q[1];"), "cx in output");
    }

    // Version-specific headers.
    assert!(qasm2.contains("OPENQASM 2.0;"));
    assert!(qasm3.contains("OPENQASM 3.0;"));
}
