// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Circuit optimization passes.
//!
//! Two passes available:
//! 1. **T-gate reduction** — algebraic cancellation of adjacent T/Tdg gates
//!    using the relation T^8 = I (mod 8 arithmetic on phase exponents).
//! 2. **ZX-calculus simplification** — via the `quizx` crate's `full_reduce`.

use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Statistics from an optimization pass.
#[derive(Debug, Clone, Default)]
pub struct OptStats {
    pub gate_count_before: usize,
    pub gate_count_after: usize,
    pub t_before: Option<usize>,
    pub t_after: Option<usize>,
}

// ── Gate counting ────────────────────────────────────────────────────────────

/// Count gate operations in a QASM string (excludes headers, measurements, barriers).
pub fn count_qasm_gates(qasm: &str) -> usize {
    qasm.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("//")
                && !line.starts_with("OPENQASM")
                && !line.starts_with("include")
                && !line.starts_with("qreg")
                && !line.starts_with("creg")
                && !line.starts_with("qubit")
                && !line.starts_with("bit")
                && !line.starts_with("measure")
                && !line.starts_with("barrier")
                && !line.starts_with("input")
        })
        .count()
}

// ── T-gate reduction ─────────────────────────────────────────────────────────

static T_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(tdg|t)\s+(q\[\d+\])\s*;$").unwrap());
static QUBIT_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"q\[\d+\]").unwrap());

/// Maps accumulated T-phase exponent (mod 8) to minimal gate sequence.
///
/// T has phase e^{iπ/4}, so T^n has phase e^{inπ/4}:
///   n=0 → identity, n=1 → T, n=2 → S, n=3 → S·T,
///   n=4 → Z, n=5 → Z·T, n=6 → Sdg, n=7 → Tdg
fn phase_to_gates(phase: u32) -> &'static [&'static str] {
    match phase % 8 {
        0 => &[],
        1 => &["t"],
        2 => &["s"],
        3 => &["s", "t"],
        4 => &["z"],
        5 => &["z", "t"],
        6 => &["sdg"],
        7 => &["tdg"],
        _ => unreachable!(),
    }
}

fn emit_phase_gates(qubit: &str, phase: u32) -> Vec<String> {
    phase_to_gates(phase)
        .iter()
        .map(|g| format!("{g} {qubit};"))
        .collect()
}

/// Reduce adjacent T/Tdg gates to their minimal algebraic form.
///
/// T gates on different qubits are commuted past each other (they are all
/// diagonal in the Z basis), allowing global cancellation within runs of
/// consecutive single-qubit T/Tdg lines.
pub fn reduce_t_gates(qasm: &str) -> (String, usize, usize) {
    let count_t = |s: &str| -> usize {
        s.lines()
            .filter(|l| {
                let t = l.trim().to_lowercase();
                t.starts_with("t ") || t.starts_with("tdg ") || t == "t" || t == "tdg"
            })
            .count()
    };

    let t_before = count_t(qasm);
    let mut qubit_phase: BTreeMap<String, u32> = BTreeMap::new();
    let mut output: Vec<String> = Vec::new();

    let flush_qubit = |q: &str, phase: u32, out: &mut Vec<String>| {
        out.extend(emit_phase_gates(q, phase));
    };

    let flush_all = |qp: &mut BTreeMap<String, u32>, out: &mut Vec<String>| {
        for (q, phase) in qp.iter() {
            out.extend(emit_phase_gates(q, *phase));
        }
        qp.clear();
    };

    for raw_line in qasm.lines() {
        let line = raw_line.trim();

        if let Some(caps) = T_LINE_RE.captures(line) {
            let gate = caps.get(1).unwrap().as_str().to_lowercase();
            let qubit = caps.get(2).unwrap().as_str().to_string();
            let delta: u32 = if gate == "t" { 1 } else { 7 }; // Tdg = T^7 mod 8
            let entry = qubit_phase.entry(qubit).or_insert(0);
            *entry = (*entry + delta) % 8;
            continue;
        }

        // For non-T lines, flush T-phases for every qubit the line touches.
        let qubits_in_line: Vec<String> = QUBIT_REF_RE
            .find_iter(line)
            .map(|m| m.as_str().to_string())
            .collect();

        // Deduplicate while preserving order.
        let mut seen = std::collections::HashSet::new();
        for q in &qubits_in_line {
            if seen.insert(q.clone()) {
                if let Some(phase) = qubit_phase.remove(q) {
                    flush_qubit(q, phase, &mut output);
                }
            }
        }

        output.push(raw_line.to_string());
    }

    flush_all(&mut qubit_phase, &mut output);

    let result = output.join("\n");
    let t_after = count_t(&result);
    (result, t_before, t_after)
}

/// Run ZX-calculus simplification with a never-worse gate-count guarantee.
///
/// If `reduce_t` is true, a T-gate cancellation pre-pass is applied first.
pub fn optimize_qasm(qasm: &str, do_reduce_t: bool) -> (String, OptStats) {
    let mut stats = OptStats::default();
    let mut working = qasm.to_string();

    if do_reduce_t {
        let (reduced, t_before, t_after) = reduce_t_gates(&working);
        working = reduced;
        stats.t_before = Some(t_before);
        stats.t_after = Some(t_after);
    }

    stats.gate_count_before = count_qasm_gates(&working);

    // Adjacent gate cancellation optimization
    let cancelled = cancel_adjacent_gates(&working);
    let cancelled_count = count_qasm_gates(&cancelled);
    
    // Only use cancellation result if it reduced gate count
    let mut current = if cancelled_count < stats.gate_count_before {
        stats.gate_count_before = cancelled_count;
        cancelled
    } else {
        working
    };

    // ZX-calculus optimization via quizx.
    match zx_optimize_qasm(&current) {
        Ok(optimized) => {
            let new_count = count_qasm_gates(&optimized);
            // Never-worse guarantee: only accept if gate count decreased or stayed equal.
            if new_count <= stats.gate_count_before {
                stats.gate_count_after = new_count;
                return (optimized, stats);
            }
        }
        Err(_) => {
            // ZX optimization failed (e.g. extraction error) — fall through to
            // return the T-reduced circuit unchanged.
        }
    }
    stats.gate_count_after = stats.gate_count_before;

    (current, stats)
}

// ── ZX-calculus optimization ─────────────────────────────────────────────────

/// Apply ZX-calculus simplification to a QASM string via quizx.
///
/// Pipeline: parse QASM → decompose to basic gates → build ZX graph →
/// `full_simp` → extract circuit → emit QASM.
///
/// Returns `Err` if parsing or circuit extraction fails (e.g. the simplified
/// graph has no valid causal flow).
pub fn zx_optimize_qasm(qasm: &str) -> Result<String, String> {
    use quizx::circuit::Circuit;
    use quizx::extract::ToCircuit;
    use quizx::simplify::full_simp;
    use quizx::vec_graph::Graph;

    let circuit = Circuit::from_qasm(qasm)?;
    let basic = circuit.to_basic_gates();
    let mut graph: Graph = basic.to_graph();
    full_simp(&mut graph);
    let optimized = graph
        .to_circuit()
        .map_err(|e| format!("ZX extraction failed: {e}"))?;
    Ok(optimized.to_qasm())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_gate_cancellation_identity() {
        // T^8 = I → eight T gates on the same qubit cancel entirely.
        let qasm = "t q[0];\nt q[0];\nt q[0];\nt q[0];\nt q[0];\nt q[0];\nt q[0];\nt q[0];";
        let (result, t_before, t_after) = reduce_t_gates(qasm);
        assert_eq!(t_before, 8);
        assert_eq!(t_after, 0);
        let trimmed: Vec<&str> = result.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(trimmed.is_empty(), "eight T gates should cancel: {result:?}");
    }

    #[test]
    fn t_gate_reduction_to_s() {
        // T^2 = S
        let qasm = "t q[0];\nt q[0];";
        let (result, t_before, t_after) = reduce_t_gates(qasm);
        assert_eq!(t_before, 2);
        assert_eq!(t_after, 0);
        assert!(result.contains("s q[0];"));
    }

    #[test]
    fn t_tdg_cancellation() {
        // T · Tdg = T^1 · T^7 = T^8 = I
        let qasm = "t q[0];\ntdg q[0];";
        let (result, t_before, t_after) = reduce_t_gates(qasm);
        assert_eq!(t_before, 2);
        assert_eq!(t_after, 0);
        let trimmed: Vec<&str> = result.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(trimmed.is_empty(), "T·Tdg should cancel: {result:?}");
    }

    #[test]
    fn non_t_gates_preserved() {
        let qasm = "h q[0];\nt q[0];\ncx q[0], q[1];\nt q[0];";
        let (result, _, _) = reduce_t_gates(qasm);
        assert!(result.contains("h q[0];"));
        assert!(result.contains("cx q[0], q[1];"));
        // The two T gates are separated by cx (which touches q[0]),
        // so they should NOT be merged.
        let t_count = result.lines().filter(|l| l.trim() == "t q[0];").count();
        assert_eq!(t_count, 2);
    }

    // ── ZX optimization tests ─────────────────────────────────────────────

    #[test]
    fn zx_hh_cancellation() {
        // H·H = I — two Hadamards on the same qubit should cancel.
        let qasm = "\
OPENQASM 2.0;\n\
include \"qelib1.inc\";\n\
qreg q[1];\n\
h q[0];\n\
h q[0];";
        let result = zx_optimize_qasm(qasm).expect("ZX optimization should succeed");
        let gate_count = count_qasm_gates(&result);
        assert_eq!(gate_count, 0, "H·H should cancel to identity, got:\n{result}");
    }

    #[test]
    fn zx_cnot_cnot_cancellation() {
        // CNOT·CNOT = I — two identical CNOTs should cancel.
        let qasm = "\
OPENQASM 2.0;\n\
include \"qelib1.inc\";\n\
qreg q[2];\n\
cx q[0], q[1];\n\
cx q[0], q[1];";
        let result = zx_optimize_qasm(qasm).expect("ZX optimization should succeed");
        let gate_count = count_qasm_gates(&result);
        assert_eq!(gate_count, 0, "CNOT·CNOT should cancel, got:\n{result}");
    }

    #[test]
    fn zx_optimize_reduces_gate_count() {
        // A circuit with redundant gates should have fewer gates after ZX optimization.
        let qasm = "\
OPENQASM 2.0;\n\
include \"qelib1.inc\";\n\
qreg q[2];\n\
h q[0];\n\
h q[0];\n\
cx q[0], q[1];\n\
cx q[0], q[1];\n\
h q[1];\n\
h q[1];";
        let before = count_qasm_gates(qasm);
        let result = zx_optimize_qasm(qasm).expect("ZX optimization should succeed");
        let after = count_qasm_gates(&result);
        assert!(after < before, "ZX should reduce gates: {before} -> {after}\n{result}");
    }

    #[test]
    fn zx_optimize_qasm_never_worse() {
        // The optimize_qasm wrapper should never produce more gates.
        let qasm = "\
OPENQASM 2.0;\n\
include \"qelib1.inc\";\n\
qreg q[2];\n\
h q[0];\n\
cx q[0], q[1];";
        let (result, stats) = optimize_qasm(qasm, false);
        assert!(
            stats.gate_count_after <= stats.gate_count_before,
            "Never-worse guarantee violated: {} -> {}",
            stats.gate_count_before,
            stats.gate_count_after
        );
        // For an already-minimal circuit, the output should still be valid QASM.
        assert!(result.contains("OPENQASM") || count_qasm_gates(&result) > 0);
    }

    #[test]
    fn zx_with_t_reduction_combined() {
        // Test the full pipeline: T-gate reduction + ZX simplification.
        let qasm = "\
OPENQASM 2.0;\n\
include \"qelib1.inc\";\n\
qreg q[2];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
t q[0];\n\
h q[1];\n\
h q[1];";
        let (result, stats) = optimize_qasm(qasm, true);
        assert_eq!(stats.t_before, Some(8));
        assert_eq!(stats.t_after, Some(0));
        assert!(
            stats.gate_count_after <= stats.gate_count_before,
            "Combined optimization should not increase gate count"
        );
        // The 8 T-gates cancel, the 2 H-gates cancel via ZX.
        assert_eq!(stats.gate_count_after, 0, "All gates should cancel, got:\n{result}");
    }

    #[test]
    fn count_gates() {
        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\nh q[0];\ncx q[0], q[1];\nmeasure q[0] -> c[0];";
        assert_eq!(count_qasm_gates(qasm), 2);
    }
}
