// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Noise-aware circuit analysis.
//!
//! Takes a compiled circuit ([`EhrenfestAst`]) and the noise constraints from
//! the Ehrenfest program, and produces a [`NoiseReport`] estimating fidelity,
//! gate times, and potential noise issues.

use std::collections::HashMap;

use crate::ast::EhrenfestAst;
use crate::cbor::{EvolutionTime, NoiseConstraint};

// ── Gate timing and error assumptions (superconducting) ────────────────────

/// Estimated single-qubit gate time in microseconds.
const SINGLE_QUBIT_GATE_TIME_US: f64 = 0.05;
/// Estimated two-qubit gate time in microseconds.
const TWO_QUBIT_GATE_TIME_US: f64 = 0.5;
/// Assumed single-qubit gate error rate.
const SINGLE_QUBIT_ERROR: f64 = 0.001;
/// Assumed two-qubit gate error rate.
const TWO_QUBIT_ERROR: f64 = 0.01;
/// Hotspot threshold: a qubit is a hotspot if it has > this factor × average.
const HOTSPOT_FACTOR: f64 = 2.0;

// ── Public types ───────────────────────────────────────────────────────────

/// Result of noise-aware circuit analysis.
#[derive(Debug, Clone)]
pub struct NoiseReport {
    /// Total circuit depth (longest qubit path in gate layers).
    pub circuit_depth: usize,
    /// Estimated total gate time in microseconds.
    pub estimated_time_us: f64,
    /// Whether the circuit fits within T2 coherence time.
    pub within_t2: bool,
    /// Whether the circuit fits within T1 relaxation time.
    pub within_t1: bool,
    /// Estimated circuit fidelity (product of gate fidelities).
    pub estimated_fidelity: f64,
    /// Per-qubit gate counts.
    pub qubit_gate_counts: Vec<usize>,
    /// Warnings about potential noise issues.
    pub warnings: Vec<NoiseWarning>,
}

#[derive(Debug, Clone)]
pub enum NoiseWarning {
    /// Circuit depth exceeds fraction of T2.
    DepthExceedsT2 {
        depth: usize,
        t2_us: f64,
        estimated_time_us: f64,
    },
    /// Circuit depth exceeds fraction of T1.
    DepthExceedsT1 {
        depth: usize,
        t1_us: f64,
        estimated_time_us: f64,
    },
    /// Estimated fidelity below gate_fidelity_min threshold.
    FidelityBelowThreshold { estimated: f64, threshold: f64 },
    /// A qubit has significantly more gates than others (potential hotspot).
    QubitHotspot {
        qubit: usize,
        gate_count: usize,
        avg_count: f64,
    },
}

// ── Public functions ───────────────────────────────────────────────────────

/// Analyse a compiled circuit against noise constraints.
pub fn analyze_noise(
    ast: &EhrenfestAst,
    noise: &NoiseConstraint,
    _evolution: &EvolutionTime,
) -> NoiseReport {
    let n_qubits = ast.n_qubits;
    let mut qubit_gate_counts = vec![0usize; n_qubits];
    let mut total_single = 0usize;
    let mut total_two = 0usize;

    for gate in &ast.gates {
        let is_two_qubit = gate.name.arity() >= 2;
        for &q in &gate.qubits {
            if q < n_qubits {
                qubit_gate_counts[q] += 1;
            }
        }
        if is_two_qubit {
            total_two += 1;
        } else {
            total_single += 1;
        }
    }

    let depth = circuit_depth(ast);

    // Estimate time: sum of sequential layer times.
    // Approximate: single-qubit layers + two-qubit layers.
    let estimated_time_us =
        (total_single as f64) * SINGLE_QUBIT_GATE_TIME_US
        + (total_two as f64) * TWO_QUBIT_GATE_TIME_US;

    let within_t2 = estimated_time_us <= noise.t2_us;
    let within_t1 = estimated_time_us <= noise.t1_us;

    // Fidelity: product of (1 - error)^n for each gate type.
    let estimated_fidelity =
        (1.0 - SINGLE_QUBIT_ERROR).powi(total_single as i32)
        * (1.0 - TWO_QUBIT_ERROR).powi(total_two as i32);

    let mut warnings = Vec::new();

    if !within_t2 {
        warnings.push(NoiseWarning::DepthExceedsT2 {
            depth,
            t2_us: noise.t2_us,
            estimated_time_us,
        });
    }

    if !within_t1 {
        warnings.push(NoiseWarning::DepthExceedsT1 {
            depth,
            t1_us: noise.t1_us,
            estimated_time_us,
        });
    }

    if let Some(threshold) = noise.gate_fidelity_min {
        if estimated_fidelity < threshold {
            warnings.push(NoiseWarning::FidelityBelowThreshold {
                estimated: estimated_fidelity,
                threshold,
            });
        }
    }

    // Hotspot detection.
    if n_qubits > 0 {
        let total_gates: usize = qubit_gate_counts.iter().sum();
        let avg = total_gates as f64 / n_qubits as f64;
        if avg > 0.0 {
            for (q, &count) in qubit_gate_counts.iter().enumerate() {
                if count as f64 > HOTSPOT_FACTOR * avg {
                    warnings.push(NoiseWarning::QubitHotspot {
                        qubit: q,
                        gate_count: count,
                        avg_count: avg,
                    });
                }
            }
        }
    }

    NoiseReport {
        circuit_depth: depth,
        estimated_time_us,
        within_t2,
        within_t1,
        estimated_fidelity,
        qubit_gate_counts,
        warnings,
    }
}

/// Calculate circuit depth using layer-based scheduling.
///
/// Gates that share no qubits can run in parallel in the same layer.
/// Returns the number of layers needed.
pub fn circuit_depth(ast: &EhrenfestAst) -> usize {
    if ast.gates.is_empty() {
        return 0;
    }

    // Per-qubit: the next available layer.
    let mut qubit_layer: Vec<usize> = vec![0; ast.n_qubits];
    let mut max_layer: usize = 0;

    for gate in &ast.gates {
        // This gate must go in a layer after all its qubits are free.
        let earliest = gate
            .qubits
            .iter()
            .filter_map(|&q| qubit_layer.get(q).copied())
            .max()
            .unwrap_or(0);

        let placed_layer = earliest;
        // Mark all qubits as occupied until layer + 1.
        for &q in &gate.qubits {
            if q < qubit_layer.len() {
                qubit_layer[q] = placed_layer + 1;
            }
        }
        if placed_layer + 1 > max_layer {
            max_layer = placed_layer + 1;
        }
    }

    max_layer
}

/// Count each gate type in the circuit.
///
/// Returns a map from gate name string to count. Uses the canonical string
/// representation as key because `GateName` does not implement `Hash`.
pub fn gate_count_by_type(ast: &EhrenfestAst) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for gate in &ast.gates {
        *counts.entry(gate.name.as_str().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Format a noise report for human-readable output on stderr.
pub fn format_report(report: &NoiseReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("  Circuit depth:      {}", report.circuit_depth));
    lines.push(format!(
        "  Estimated time:     {:.2} \u{00b5}s",
        report.estimated_time_us
    ));
    lines.push(format!(
        "  Estimated fidelity: {:.6}",
        report.estimated_fidelity
    ));
    lines.push(format!(
        "  Within T1:          {}",
        if report.within_t1 { "yes" } else { "NO" }
    ));
    lines.push(format!(
        "  Within T2:          {}",
        if report.within_t2 { "yes" } else { "NO" }
    ));
    for w in &report.warnings {
        match w {
            NoiseWarning::DepthExceedsT2 {
                depth,
                t2_us,
                estimated_time_us,
            } => {
                lines.push(format!(
                    "  WARNING: circuit time ({estimated_time_us:.2} us) exceeds T2 ({t2_us:.2} us) at depth {depth}"
                ));
            }
            NoiseWarning::DepthExceedsT1 {
                depth,
                t1_us,
                estimated_time_us,
            } => {
                lines.push(format!(
                    "  WARNING: circuit time ({estimated_time_us:.2} us) exceeds T1 ({t1_us:.2} us) at depth {depth}"
                ));
            }
            NoiseWarning::FidelityBelowThreshold {
                estimated,
                threshold,
            } => {
                lines.push(format!(
                    "  WARNING: fidelity ({estimated:.6}) below threshold ({threshold:.6})"
                ));
            }
            NoiseWarning::QubitHotspot {
                qubit,
                gate_count,
                avg_count,
            } => {
                lines.push(format!(
                    "  WARNING: qubit {qubit} hotspot ({gate_count} gates, avg {avg_count:.1})"
                ));
            }
        }
    }
    lines.join("\n")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Gate, GateName};

    fn empty_ast(n_qubits: usize) -> EhrenfestAst {
        EhrenfestAst {
            name: "test".into(),
            n_qubits,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        }
    }

    fn default_noise() -> NoiseConstraint {
        NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        }
    }

    fn default_evolution() -> EvolutionTime {
        EvolutionTime {
            total_us: 1.0,
            steps: 10,
            dt_us: 0.1,
        }
    }

    #[test]
    fn empty_circuit_depth_zero_fidelity_one() {
        let ast = empty_ast(2);
        let report = analyze_noise(&ast, &default_noise(), &default_evolution());
        assert_eq!(report.circuit_depth, 0);
        assert!((report.estimated_fidelity - 1.0).abs() < 1e-10);
        assert_eq!(report.estimated_time_us, 0.0);
        assert!(report.within_t1);
        assert!(report.within_t2);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn single_h_gate_depth_one_high_fidelity() {
        let mut ast = empty_ast(1);
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        });
        let report = analyze_noise(&ast, &default_noise(), &default_evolution());
        assert_eq!(report.circuit_depth, 1);
        assert!(report.estimated_fidelity > 0.99);
        assert!(report.within_t1);
        assert!(report.within_t2);
    }

    #[test]
    fn deep_cx_circuit_within_t2_depends_on_t2() {
        let mut ast = empty_ast(2);
        // 200 CX gates in series on qubits 0,1.
        for _ in 0..200 {
            ast.gates.push(Gate {
                name: GateName::Cx,
                qubits: vec![0, 1],
                params: vec![],
            });
        }
        // Each CX = 0.5us, 200 CX = 100us total time.
        let short_t2 = NoiseConstraint {
            t1_us: 200.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        };
        let report = analyze_noise(&ast, &short_t2, &default_evolution());
        assert_eq!(report.circuit_depth, 200);
        assert!(!report.within_t2);
        assert!(report.within_t1);

        // With longer T2, should be within.
        let long_t2 = NoiseConstraint {
            t1_us: 200.0,
            t2_us: 200.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        };
        let report2 = analyze_noise(&ast, &long_t2, &default_evolution());
        assert!(report2.within_t2);
    }

    #[test]
    fn hotspot_detection_all_gates_on_qubit_zero() {
        let mut ast = empty_ast(4);
        // 20 gates all on qubit 0, none on others.
        for _ in 0..20 {
            ast.gates.push(Gate {
                name: GateName::H,
                qubits: vec![0],
                params: vec![],
            });
        }
        let report = analyze_noise(&ast, &default_noise(), &default_evolution());
        // avg = 20/4 = 5, qubit 0 has 20 > 2*5 = 10 → hotspot.
        let hotspots: Vec<_> = report
            .warnings
            .iter()
            .filter(|w| matches!(w, NoiseWarning::QubitHotspot { .. }))
            .collect();
        assert_eq!(hotspots.len(), 1);
        if let NoiseWarning::QubitHotspot {
            qubit, gate_count, ..
        } = &hotspots[0]
        {
            assert_eq!(*qubit, 0);
            assert_eq!(*gate_count, 20);
        }
    }

    #[test]
    fn fidelity_below_threshold_warning() {
        let mut ast = empty_ast(2);
        // Many two-qubit gates to degrade fidelity.
        for _ in 0..100 {
            ast.gates.push(Gate {
                name: GateName::Cx,
                qubits: vec![0, 1],
                params: vec![],
            });
        }
        let noise = NoiseConstraint {
            t1_us: 1000.0,
            t2_us: 1000.0,
            gate_fidelity_min: Some(0.99),
            readout_fidelity_min: None,
        };
        let report = analyze_noise(&ast, &noise, &default_evolution());
        // (1-0.01)^100 ≈ 0.366 < 0.99
        assert!(report.estimated_fidelity < 0.99);
        let fidelity_warnings: Vec<_> = report
            .warnings
            .iter()
            .filter(|w| matches!(w, NoiseWarning::FidelityBelowThreshold { .. }))
            .collect();
        assert_eq!(fidelity_warnings.len(), 1);
    }

    #[test]
    fn circuit_depth_parallel_gates() {
        let mut ast = empty_ast(4);
        // H on q0 and H on q1 can be parallel → depth 1.
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        });
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![1],
            params: vec![],
        });
        assert_eq!(circuit_depth(&ast), 1);

        // Add CX(0,1) → must be layer 1, so depth 2.
        ast.gates.push(Gate {
            name: GateName::Cx,
            qubits: vec![0, 1],
            params: vec![],
        });
        assert_eq!(circuit_depth(&ast), 2);
    }

    #[test]
    fn gate_count_by_type_counts_correctly() {
        let mut ast = empty_ast(2);
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![0],
            params: vec![],
        });
        ast.gates.push(Gate {
            name: GateName::H,
            qubits: vec![1],
            params: vec![],
        });
        ast.gates.push(Gate {
            name: GateName::Cx,
            qubits: vec![0, 1],
            params: vec![],
        });
        let counts = gate_count_by_type(&ast);
        assert_eq!(counts.get("h"), Some(&2));
        assert_eq!(counts.get("cx"), Some(&1));
        assert_eq!(counts.get("x"), None);
    }
}
