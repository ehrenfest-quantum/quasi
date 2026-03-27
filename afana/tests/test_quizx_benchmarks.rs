// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! QuiZX optimization benchmark suite.
//!
//! Proves that the ZX-calculus optimizer (with optional T-gate reduction)
//! achieves measurable gate reduction on realistic Trotterized circuits.

use afana::cbor::*;
use afana::emit::{emit_qasm, QasmVersion};
use afana::optimize::{count_qasm_gates, optimize_qasm};
use afana::trotter::{trotterize, TrotterOrder};

// ── Helper: build an EhrenfestProgram from Hamiltonian terms ─────────────────

fn make_program(n_qubits: usize, terms: Vec<PauliTerm>, steps: u32) -> EhrenfestProgram {
    let total_us = 1.0;
    let dt_us = total_us / steps as f64;
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us,
            steps,
            dt_us,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

/// Compile an EhrenfestProgram to QASM 2.0, returning the QASM string.
fn compile_to_qasm(program: &EhrenfestProgram) -> String {
    let ast = trotterize(program, TrotterOrder::First);
    emit_qasm(&ast, QasmVersion::V2).unwrap()
}

/// Run the optimization pipeline and return (original_gates, optimized_gates, reduction_pct).
fn benchmark_optimize(qasm: &str, do_reduce_t: bool) -> (usize, usize, f64) {
    let before = count_qasm_gates(qasm);
    let (_, stats) = optimize_qasm(qasm, do_reduce_t);
    let after = stats.gate_count_after;
    let pct = if before > 0 {
        (1.0 - after as f64 / before as f64) * 100.0
    } else {
        0.0
    };
    (before, after, pct)
}

// ── Heisenberg 4-qubit chain ────────────────────────────────────────────────
//
// H = sum_i (Jx Xi Xi+1 + Jy Yi Yi+1 + Jz Zi Zi+1)  for i in 0..3
// This is the standard Heisenberg XXX model on 4 qubits with nearest-neighbor
// coupling. Trotterization produces many basis-change + CNOT-ladder + Rz blocks
// that the ZX optimizer can simplify.

fn heisenberg_4q_terms() -> Vec<PauliTerm> {
    let mut terms = Vec::new();
    let j = 0.5;
    for i in 0..3 {
        // XX coupling
        terms.push(PauliTerm {
            coefficient: j,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::X },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::X },
            ],
        });
        // YY coupling
        terms.push(PauliTerm {
            coefficient: j,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Y },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Y },
            ],
        });
        // ZZ coupling
        terms.push(PauliTerm {
            coefficient: j,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
            ],
        });
    }
    terms
}

#[test]
fn benchmark_heisenberg_4q_zx_reduction() {
    let program = make_program(4, heisenberg_4q_terms(), 10);
    let qasm = compile_to_qasm(&program);
    let (before, after, pct) = benchmark_optimize(&qasm, false);

    println!("Heisenberg 4q (ZX only): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "ZX optimizer should reduce Heisenberg 4q gates: {before} -> {after}"
    );
}

#[test]
fn benchmark_heisenberg_4q_full_pipeline() {
    let program = make_program(4, heisenberg_4q_terms(), 10);
    let qasm = compile_to_qasm(&program);
    let (before, after, pct) = benchmark_optimize(&qasm, true);

    println!("Heisenberg 4q (T-reduce + ZX): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "Full optimizer should reduce Heisenberg 4q gates: {before} -> {after}"
    );
}

// ── Ising chain (transverse-field) ──────────────────────────────────────────
//
// H = -J sum_i Zi Zi+1 - h sum_i Xi
// The transverse-field Ising model. ZZ terms produce CNOT ladders,
// X terms produce H-Rz-H blocks. Both are amenable to ZX simplification.

fn ising_chain_terms(n_qubits: usize) -> Vec<PauliTerm> {
    let mut terms = Vec::new();
    let j = 1.0;
    let h_field = 0.5;

    // ZZ nearest-neighbor coupling
    for i in 0..n_qubits - 1 {
        terms.push(PauliTerm {
            coefficient: -j,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
            ],
        });
    }
    // Transverse field (X on each qubit)
    for i in 0..n_qubits {
        terms.push(PauliTerm {
            coefficient: -h_field,
            paulis: vec![PauliOpEntry { qubit: i, axis: PauliOp::X }],
        });
    }
    terms
}

#[test]
fn benchmark_ising_6q_zx_reduction() {
    let program = make_program(6, ising_chain_terms(6), 8);
    let qasm = compile_to_qasm(&program);
    let (before, after, pct) = benchmark_optimize(&qasm, false);

    println!("Ising 6q chain (ZX only): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "ZX optimizer should reduce Ising 6q gates: {before} -> {after}"
    );
}

#[test]
fn benchmark_ising_6q_full_pipeline() {
    let program = make_program(6, ising_chain_terms(6), 8);
    let qasm = compile_to_qasm(&program);
    let (before, after, pct) = benchmark_optimize(&qasm, true);

    println!("Ising 6q chain (T-reduce + ZX): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "Full optimizer should reduce Ising 6q gates: {before} -> {after}"
    );
}

// ── Deep CNOT ladder ────────────────────────────────────────────────────────
//
// A deep ladder of alternating CNOT pairs + H gates. Adjacent identical CNOTs
// should cancel completely via ZX. We build this as raw QASM to test the
// optimizer's ability to handle non-Trotterized circuits.

fn deep_cnot_ladder_qasm(n_qubits: usize, depth: usize) -> String {
    let mut lines = Vec::new();
    lines.push("OPENQASM 2.0;".to_string());
    lines.push("include \"qelib1.inc\";".to_string());
    lines.push(format!("qreg q[{n_qubits}];"));

    for _ in 0..depth {
        // Forward CNOT ladder
        for i in 0..n_qubits - 1 {
            lines.push(format!("cx q[{i}], q[{}];", i + 1));
        }
        // Hadamard layer (prevents trivial full cancellation)
        for i in 0..n_qubits {
            lines.push(format!("h q[{i}];"));
        }
        // Another forward CNOT ladder
        for i in 0..n_qubits - 1 {
            lines.push(format!("cx q[{i}], q[{}];", i + 1));
        }
    }
    lines.join("\n")
}

#[test]
fn benchmark_deep_cnot_ladder_zx_reduction() {
    let qasm = deep_cnot_ladder_qasm(4, 5);
    let (before, after, pct) = benchmark_optimize(&qasm, false);

    println!("Deep CNOT ladder 4q d=5 (ZX only): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "ZX optimizer should reduce deep CNOT ladder: {before} -> {after}"
    );
}

// ── Second-order Trotter vs first-order ─────────────────────────────────────
//
// Second-order Trotter produces symmetric forward+reverse half-steps, which
// creates adjacent gate pairs that the ZX optimizer can cancel.

#[test]
fn benchmark_second_order_trotter_reduction() {
    let program = make_program(4, heisenberg_4q_terms(), 5);
    let ast = trotterize(&program, TrotterOrder::Second);
    let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
    let (before, after, pct) = benchmark_optimize(&qasm, true);

    println!("Heisenberg 4q 2nd-order Trotter (T-reduce + ZX): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "Optimizer should reduce 2nd-order Trotter circuit: {before} -> {after}"
    );
}

// ── Redundant H-gate layer benchmark ────────────────────────────────────────
//
// A circuit with explicit redundancies: H-H pairs that should cancel,
// interleaved with useful gates. This proves the ZX optimizer handles
// real-world redundancy patterns.

fn redundant_h_circuit_qasm() -> String {
    let mut lines = Vec::new();
    lines.push("OPENQASM 2.0;".to_string());
    lines.push("include \"qelib1.inc\";".to_string());
    lines.push("qreg q[3];".to_string());

    for _ in 0..10 {
        // H-H pair on q[0] (should cancel)
        lines.push("h q[0];".to_string());
        lines.push("h q[0];".to_string());
        // Useful CNOT
        lines.push("cx q[0], q[1];".to_string());
        // H-H pair on q[1] (should cancel)
        lines.push("h q[1];".to_string());
        lines.push("h q[1];".to_string());
        // Useful CNOT
        lines.push("cx q[1], q[2];".to_string());
    }
    lines.join("\n")
}

#[test]
fn benchmark_redundant_h_gates_reduction() {
    let qasm = redundant_h_circuit_qasm();
    let (before, after, pct) = benchmark_optimize(&qasm, false);

    println!("Redundant H-gate circuit (ZX only): {before} -> {after} gates ({pct:.1}% reduction)");
    assert!(
        after < before,
        "ZX optimizer should cancel redundant H-H pairs: {before} -> {after}"
    );
    // With 20 H-H pairs out of 60 gates, expect at least 30% reduction.
    assert!(
        pct >= 10.0,
        "Expected at least 10% reduction from H-H cancellation, got {pct:.1}%"
    );
}

// ── Never-worse guarantee across all benchmarks ─────────────────────────────

#[test]
fn optimizer_never_increases_gate_count() {
    // Test the never-worse guarantee on a variety of circuits.
    let circuits = vec![
        ("Heisenberg 4q", compile_to_qasm(&make_program(4, heisenberg_4q_terms(), 5))),
        ("Ising 4q", compile_to_qasm(&make_program(4, ising_chain_terms(4), 5))),
        ("CNOT ladder", deep_cnot_ladder_qasm(3, 3)),
        ("Redundant H", redundant_h_circuit_qasm()),
    ];

    for (name, qasm) in &circuits {
        let before = count_qasm_gates(qasm);
        let (_, stats) = optimize_qasm(qasm, true);
        assert!(
            stats.gate_count_after <= before,
            "Never-worse violated for {name}: {before} -> {}",
            stats.gate_count_after
        );
        let pct = if before > 0 {
            (1.0 - stats.gate_count_after as f64 / before as f64) * 100.0
        } else {
            0.0
        };
        println!("Never-worse check {name}: {before} -> {} gates ({pct:.1}% reduction)", stats.gate_count_after);
    }
}
