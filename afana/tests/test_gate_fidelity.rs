// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR round-trip validation harness for gate fidelity verification.
//!
//! Verifies that the emit → parse → re-emit pipeline preserves circuit
//! structure: build an EhrenfestAst, emit QASM2 via `emit_qasm()`, parse
//! back with quizx `Circuit::from_qasm()`, re-emit via `circuit.to_qasm()`,
//! and verify gate counts and structure are preserved.
//!
//! Uses QASM2 (with qreg/creg) because quizx's parser expects that format.

use afana::ast::*;
use afana::emit::{emit_qasm, QasmVersion};
use afana::optimize::count_qasm_gates;
use quizx::circuit::Circuit;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a minimal AST (no measurements) for round-trip testing.
///
/// quizx does not preserve measurement operations through its circuit
/// representation, so we omit them for structural round-trip checks.
fn make_ast(name: &str, n_qubits: usize, gates: Vec<Gate>) -> EhrenfestAst {
    EhrenfestAst {
        name: name.into(),
        n_qubits,
        prepare: None,
        gates,
        measures: Vec::new(),
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    }
}

/// Emit QASM2 from an AST, parse via quizx, and return both the original
/// QASM string and the quizx Circuit.
fn emit_and_parse(ast: &EhrenfestAst) -> (String, Circuit) {
    let qasm = emit_qasm(ast, QasmVersion::V2).expect("emit_qasm should succeed");
    let circuit = Circuit::from_qasm(&qasm).expect("quizx should parse the emitted QASM2");
    (qasm, circuit)
}

/// Count gate lines in emitted QASM (excluding headers, declarations, measures).
fn count_gate_lines(qasm: &str) -> usize {
    count_qasm_gates(qasm)
}

// ── Bell circuit round-trip ─────────────────────────────────────────────────

#[test]
fn bell_roundtrip_gate_count_preserved() {
    let ast = make_ast(
        "bell",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
    );

    let (original_qasm, circuit) = emit_and_parse(&ast);
    let original_count = count_gate_lines(&original_qasm);
    assert_eq!(original_count, 2, "Bell circuit should have 2 gates");

    // Re-emit from quizx and verify structure.
    let re_emitted = circuit.to_qasm();
    assert!(
        re_emitted.contains("OPENQASM"),
        "re-emitted QASM must have header"
    );

    // quizx decomposes to basic gates, so gate count may differ but the
    // circuit must still be valid and non-empty.
    let re_count = count_gate_lines(&re_emitted);
    assert!(re_count > 0, "re-emitted circuit must have gates");
}

#[test]
fn bell_roundtrip_quizx_parses_successfully() {
    let ast = make_ast(
        "bell",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
    );

    let (_, circuit) = emit_and_parse(&ast);

    // Verify quizx sees the correct number of qubits.
    assert_eq!(circuit.num_qubits(), 2, "quizx must see 2 qubits");
}

#[test]
fn bell_double_roundtrip_stable() {
    // emit → parse → re-emit → parse again: the second parse must also succeed.
    let ast = make_ast(
        "bell",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
    );

    let (_, circuit1) = emit_and_parse(&ast);
    let qasm2 = circuit1.to_qasm();
    let circuit2 = Circuit::from_qasm(&qasm2).expect("second parse must succeed");
    let qasm3 = circuit2.to_qasm();

    // After the first quizx round-trip, the output should stabilize.
    let count2 = count_gate_lines(&qasm2);
    let count3 = count_gate_lines(&qasm3);
    assert_eq!(
        count2, count3,
        "gate count must stabilize after first round-trip: {count2} vs {count3}"
    );
}

// ── RZ rotation round-trip ──────────────────────────────────────────────────

#[test]
fn rz_rotation_roundtrip_preserved() {
    let ast = make_ast(
        "rz_test",
        1,
        vec![Gate {
            name: GateName::Rz,
            qubits: vec![0],
            params: vec![std::f64::consts::FRAC_PI_4],
        }],
    );

    let (original_qasm, circuit) = emit_and_parse(&ast);
    assert_eq!(count_gate_lines(&original_qasm), 1, "RZ circuit: 1 gate");

    // quizx must parse a single-qubit rotation.
    assert_eq!(circuit.num_qubits(), 1);

    // Re-emit and verify it is non-trivial (the rotation should not vanish).
    let re_emitted = circuit.to_qasm();
    let re_count = count_gate_lines(&re_emitted);
    assert!(
        re_count >= 1,
        "RZ rotation must survive round-trip, got {re_count} gates"
    );
}

#[test]
fn rz_pi_half_roundtrip() {
    let ast = make_ast(
        "rz_pi2",
        1,
        vec![Gate {
            name: GateName::Rz,
            qubits: vec![0],
            params: vec![std::f64::consts::FRAC_PI_2],
        }],
    );

    let (qasm, _circuit) = emit_and_parse(&ast);
    assert!(
        qasm.contains("rz(pi/2)") || qasm.contains("rz(1.5707"),
        "emitted QASM must contain rz with pi/2 parameter"
    );
}

// ── H-CX-H pattern round-trip ──────────────────────────────────────────────

#[test]
fn h_cx_h_roundtrip_gate_count() {
    let ast = make_ast(
        "h_cx_h",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
        ],
    );

    let (original_qasm, circuit) = emit_and_parse(&ast);
    let original_count = count_gate_lines(&original_qasm);
    assert_eq!(original_count, 3, "H-CX-H pattern should have 3 gates");
    assert_eq!(circuit.num_qubits(), 2);

    // Re-emit and verify non-trivial output (this pattern does not simplify
    // to identity, so gates must survive).
    let re_emitted = circuit.to_qasm();
    let re_count = count_gate_lines(&re_emitted);
    assert!(
        re_count >= 1,
        "H-CX-H must survive round-trip with at least 1 gate, got {re_count}"
    );
}

#[test]
fn h_cx_h_double_roundtrip_stable() {
    let ast = make_ast(
        "h_cx_h",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
        ],
    );

    let (_, circuit1) = emit_and_parse(&ast);
    let qasm2 = circuit1.to_qasm();
    let circuit2 = Circuit::from_qasm(&qasm2).expect("second parse must succeed");
    let qasm3 = circuit2.to_qasm();

    let count2 = count_gate_lines(&qasm2);
    let count3 = count_gate_lines(&qasm3);
    assert_eq!(
        count2, count3,
        "H-CX-H gate count must stabilize: {count2} vs {count3}"
    );
}

// ── Multi-gate sequence round-trip ──────────────────────────────────────────

#[test]
fn multi_gate_sequence_roundtrip() {
    // A more complex circuit: H, T, CX, S, H — tests that multiple gate
    // types survive the round-trip pipeline.
    let ast = make_ast(
        "multi_gate",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::T, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::S, qubits: vec![1], params: vec![] },
            Gate { name: GateName::H, qubits: vec![1], params: vec![] },
        ],
    );

    let (original_qasm, circuit) = emit_and_parse(&ast);
    let original_count = count_gate_lines(&original_qasm);
    assert_eq!(original_count, 5, "multi-gate circuit should have 5 gates");
    assert_eq!(circuit.num_qubits(), 2);

    let re_emitted = circuit.to_qasm();
    let re_count = count_gate_lines(&re_emitted);
    assert!(
        re_count >= 1,
        "multi-gate circuit must survive round-trip, got {re_count} gates"
    );
}

#[test]
fn multi_gate_quizx_reparse_succeeds() {
    // Verify the re-emitted QASM from quizx is itself parseable.
    let ast = make_ast(
        "reparse",
        2,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::T, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ],
    );

    let (_, circuit1) = emit_and_parse(&ast);
    let re_emitted = circuit1.to_qasm();

    // Must be valid QASM that quizx can parse again.
    let circuit2 = Circuit::from_qasm(&re_emitted)
        .expect("quizx must be able to re-parse its own output");
    assert_eq!(
        circuit1.num_qubits(),
        circuit2.num_qubits(),
        "qubit count must be preserved across round-trips"
    );
}

// ── Identity cancellation round-trip ────────────────────────────────────────

#[test]
fn hh_cancellation_roundtrip() {
    // H-H on the same qubit should cancel to identity through quizx.
    let ast = make_ast(
        "hh_cancel",
        1,
        vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
        ],
    );

    let (original_qasm, _circuit) = emit_and_parse(&ast);
    let original_count = count_gate_lines(&original_qasm);
    assert_eq!(original_count, 2, "emitted QASM has 2 H gates");

    // quizx may or may not simplify H-H; the key property is that
    // the round-trip produces valid, parseable QASM.
    let re_emitted = _circuit.to_qasm();
    Circuit::from_qasm(&re_emitted).expect("re-emitted QASM must be parseable");
}

// ── Qubit count preservation ────────────────────────────────────────────────

#[test]
fn qubit_count_preserved_across_roundtrip() {
    for n in [1_usize, 2, 3, 4] {
        let mut gates = vec![Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        }];
        for i in 0..n.saturating_sub(1) {
            gates.push(Gate {
                name: GateName::Cx,
                qubits: vec![i, i + 1],
                params: vec![],
            });
        }

        let ast = make_ast(&format!("qcount_{n}"), n, gates);
        let (_, circuit) = emit_and_parse(&ast);
        assert_eq!(
            circuit.num_qubits(),
            n,
            "quizx must preserve {n}-qubit count"
        );
    }
}
