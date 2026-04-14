// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Temporal scheduling and coherence budgeting (Phase D).
//!
//! Tracks T2 deadlines per qubit and detects circuits that exceed the
//! coherence window. Provides recommendations: proceed, warn, or exceed
//! (with suggested split count and minimum T2 requirement).

use crate::calibration::BackendCalibration;
use crate::partition::PartitionPlan;

/// Gate timing assumptions for different gate types.
#[derive(Debug, Clone)]
pub struct GateTiming {
    /// Single-qubit gate time in microseconds.
    pub single_qubit_us: f64,
    /// Two-qubit gate time in microseconds.
    pub two_qubit_us: f64,
    /// Measurement time in microseconds.
    pub measurement_us: f64,
}

impl Default for GateTiming {
    fn default() -> Self {
        // Typical superconducting QPU timings
        Self {
            single_qubit_us: 0.05,
            two_qubit_us: 0.5,
            measurement_us: 1.0,
        }
    }
}

/// Result of coherence analysis for a circuit on a specific backend region.
#[derive(Debug, Clone)]
pub struct CoherenceAnalysis {
    /// Estimated total circuit execution time in microseconds.
    pub circuit_time_us: f64,
    /// Minimum T2 across the assigned qubits.
    pub min_t2_us: f64,
    /// Ratio: circuit_time / min_t2. Values > 1.0 mean the circuit exceeds coherence.
    pub t2_ratio: f64,
    /// Whether the circuit fits within the coherence window.
    pub within_coherence: bool,
    /// Per-qubit coherence margins (T2 - qubit_busy_time). Negative = exceeded.
    pub qubit_margins: Vec<(u32, f64)>,
    /// Recommended action.
    pub recommendation: CoherenceAction,
}

/// Recommended action based on coherence analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum CoherenceAction {
    /// Circuit fits comfortably. Proceed.
    Proceed,
    /// Circuit is tight but feasible. Warn.
    Warn { t2_ratio: f64 },
    /// Circuit exceeds coherence. Must take action.
    Exceed {
        /// Suggested: reroute to a backend with longer coherence.
        suggest_backend_t2_min: f64,
        /// Suggested: split into N sub-circuits.
        suggest_split_count: u32,
    },
}

/// Analyze whether a circuit fits within the coherence window of assigned qubits.
///
/// Estimates circuit execution time from gate counts and timing parameters,
/// then compares against the minimum T2 across the assigned qubits.
///
/// # Arguments
///
/// * `circuit_depth` - Number of gate layers in the circuit.
/// * `n_single_qubit_gates` - Total single-qubit gate count.
/// * `n_two_qubit_gates` - Total two-qubit gate count.
/// * `n_measurements` - Total measurement count.
/// * `qubit_calibrations` - Per-qubit (physical_qubit_id, T2 in microseconds).
/// * `timing` - Gate timing assumptions.
pub fn analyze_coherence(
    _circuit_depth: u32,
    n_single_qubit_gates: u32,
    n_two_qubit_gates: u32,
    n_measurements: u32,
    qubit_calibrations: &[(u32, f64)],
    timing: &GateTiming,
) -> CoherenceAnalysis {
    // 1. Estimate circuit time from gate counts and timing.
    let circuit_time_us = (n_single_qubit_gates as f64) * timing.single_qubit_us
        + (n_two_qubit_gates as f64) * timing.two_qubit_us
        + (n_measurements as f64) * timing.measurement_us;

    // 2. Find minimum T2 across assigned qubits.
    let min_t2_us = qubit_calibrations
        .iter()
        .map(|&(_, t2)| t2)
        .fold(f64::INFINITY, f64::min);

    // Handle edge case: no qubits assigned.
    let min_t2_us = if min_t2_us.is_infinite() {
        0.0
    } else {
        min_t2_us
    };

    // 3. Compute ratio and per-qubit margins.
    let t2_ratio = if min_t2_us > 0.0 {
        circuit_time_us / min_t2_us
    } else if circuit_time_us == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };

    let within_coherence = t2_ratio < 1.0;

    let qubit_margins: Vec<(u32, f64)> = qubit_calibrations
        .iter()
        .map(|&(qubit_id, t2)| (qubit_id, t2 - circuit_time_us))
        .collect();

    // 4. Determine recommendation.
    let recommendation = if t2_ratio < 0.5 {
        CoherenceAction::Proceed
    } else if t2_ratio < 1.0 {
        CoherenceAction::Warn { t2_ratio }
    } else {
        let suggest_split = suggest_split_count(circuit_time_us, min_t2_us, 0.7);
        let suggest_t2 = circuit_time_us * 1.5;
        CoherenceAction::Exceed {
            suggest_backend_t2_min: suggest_t2,
            suggest_split_count: suggest_split,
        }
    };

    CoherenceAnalysis {
        circuit_time_us,
        min_t2_us,
        t2_ratio,
        within_coherence,
        qubit_margins,
        recommendation,
    }
}

/// Estimate how many sub-circuits a circuit should be split into
/// to fit within coherence.
///
/// Each sub-circuit must fit in `t2 * safety_margin`. The safety margin
/// (typically 0.7) reserves headroom for classical overhead between slices.
///
/// Returns at least 1.
pub fn suggest_split_count(circuit_time_us: f64, t2_us: f64, safety_margin: f64) -> u32 {
    if t2_us <= 0.0 || safety_margin <= 0.0 {
        return 1;
    }
    let effective_window = t2_us * safety_margin;
    if effective_window <= 0.0 || circuit_time_us <= 0.0 {
        return 1;
    }
    (circuit_time_us / effective_window).ceil().max(1.0) as u32
}

/// Given a partition plan and calibration, analyze coherence for each program.
///
/// Returns a vector of `(program_id, CoherenceAnalysis)` for each allocated
/// program in the partition.
///
/// # Arguments
///
/// * `partition` - The partition plan from Phase C.
/// * `program_depths` - Per-program circuit info as
///   `(id, depth, single_qubit_gates, two_qubit_gates, measurements)`.
/// * `calibration` - Backend calibration with per-qubit T2 data.
/// * `timing` - Gate timing assumptions.
pub fn analyze_partition_coherence(
    partition: &PartitionPlan,
    program_depths: &[(String, u32, u32, u32, u32)],
    calibration: &BackendCalibration,
    timing: &GateTiming,
) -> Vec<(String, CoherenceAnalysis)> {
    partition
        .allocations
        .iter()
        .filter_map(|alloc| {
            // Find matching program depth info.
            let info = program_depths
                .iter()
                .find(|(id, _, _, _, _)| id == &alloc.program_id)?;

            let (_, depth, n_1q, n_2q, n_meas) = info;

            // Build qubit calibrations from assigned physical qubits.
            let qubit_cals: Vec<(u32, f64)> = alloc
                .physical_qubits
                .iter()
                .filter_map(|&q| {
                    calibration
                        .qubits
                        .get(q as usize)
                        .map(|qc| (q, qc.t2))
                })
                .collect();

            let analysis =
                analyze_coherence(*depth, *n_1q, *n_2q, *n_meas, &qubit_cals, timing);

            Some((alloc.program_id.clone(), analysis))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::BackendCalibration;
    use crate::partition::{partition_qpu, ProgramRequest};

    fn linear_edges(n: u32) -> Vec<(u32, u32)> {
        (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
    }

    #[test]
    fn circuit_well_within_t2_proceeds() {
        // 10 single-qubit gates at 0.05us = 0.5us total.
        // T2 = 150us. Ratio = 0.5/150 = 0.003 → Proceed.
        let cals = vec![(0, 150.0), (1, 150.0)];
        let result = analyze_coherence(10, 10, 0, 0, &cals, &GateTiming::default());

        assert!(result.within_coherence);
        assert!(result.t2_ratio < 0.5);
        assert_eq!(result.recommendation, CoherenceAction::Proceed);
        assert!((result.circuit_time_us - 0.5).abs() < 1e-9);
    }

    #[test]
    fn circuit_at_sixty_percent_t2_warns() {
        // Need circuit_time / min_t2 to be between 0.5 and 1.0.
        // 120 two-qubit gates at 0.5us = 60us. T2 = 100us. Ratio = 0.6 → Warn.
        let cals = vec![(0, 100.0), (1, 100.0)];
        let result = analyze_coherence(120, 0, 120, 0, &cals, &GateTiming::default());

        assert!(result.within_coherence);
        assert!(result.t2_ratio >= 0.5);
        assert!(result.t2_ratio < 1.0);
        match &result.recommendation {
            CoherenceAction::Warn { t2_ratio } => {
                assert!((*t2_ratio - 0.6).abs() < 1e-9);
            }
            other => panic!("expected Warn, got {:?}", other),
        }
    }

    #[test]
    fn circuit_exceeding_t2_with_correct_split_suggestion() {
        // 400 two-qubit gates at 0.5us = 200us. T2 = 100us. Ratio = 2.0 → Exceed.
        let cals = vec![(0, 100.0), (1, 100.0)];
        let result = analyze_coherence(400, 0, 400, 0, &cals, &GateTiming::default());

        assert!(!result.within_coherence);
        assert!(result.t2_ratio >= 1.0);
        match &result.recommendation {
            CoherenceAction::Exceed {
                suggest_backend_t2_min,
                suggest_split_count,
            } => {
                // suggest_t2 = 200 * 1.5 = 300
                assert!((*suggest_backend_t2_min - 300.0).abs() < 1e-9);
                // split: 200 / (100 * 0.7) = 200/70 ≈ 2.86 → ceil = 3
                assert_eq!(*suggest_split_count, 3);
            }
            other => panic!("expected Exceed, got {:?}", other),
        }
    }

    #[test]
    fn suggest_split_count_250us_on_150us_t2() {
        // 250 / (150 * 0.7) = 250 / 105 ≈ 2.38 → ceil = 3
        let count = suggest_split_count(250.0, 150.0, 0.7);
        assert_eq!(count, 3);
    }

    #[test]
    fn suggest_split_count_fits_returns_one() {
        // 50 / (150 * 0.7) = 50 / 105 ≈ 0.48 → ceil = 1
        let count = suggest_split_count(50.0, 150.0, 0.7);
        assert_eq!(count, 1);
    }

    #[test]
    fn suggest_split_count_zero_t2_returns_one() {
        let count = suggest_split_count(100.0, 0.0, 0.7);
        assert_eq!(count, 1);
    }

    #[test]
    fn per_qubit_margins_worst_qubit_identified() {
        // Qubit 0: T2=50, Qubit 1: T2=200, Qubit 2: T2=30 (worst).
        // Circuit time: 40us (80 single-qubit gates).
        let cals = vec![(0, 50.0), (1, 200.0), (2, 30.0)];
        let result = analyze_coherence(80, 80, 0, 0, &cals, &GateTiming::default());

        // Circuit time = 80 * 0.05 = 4.0us
        assert!((result.circuit_time_us - 4.0).abs() < 1e-9);
        assert!((result.min_t2_us - 30.0).abs() < 1e-9);

        // Check margins: T2 - circuit_time for each qubit.
        let margin_q0 = result
            .qubit_margins
            .iter()
            .find(|&&(q, _)| q == 0)
            .map(|&(_, m)| m)
            .unwrap();
        let margin_q2 = result
            .qubit_margins
            .iter()
            .find(|&&(q, _)| q == 2)
            .map(|&(_, m)| m)
            .unwrap();

        assert!((margin_q0 - 46.0).abs() < 1e-9); // 50 - 4 = 46
        assert!((margin_q2 - 26.0).abs() < 1e-9); // 30 - 4 = 26
        // Qubit 2 has the smallest margin.
        assert!(margin_q2 < margin_q0);
    }

    #[test]
    fn analyze_partition_coherence_mixed_results() {
        let total = 50u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "fast".into(),
                n_qubits: 5,
                priority: 2,
            },
            ProgramRequest {
                id: "slow".into(),
                n_qubits: 5,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 2);

        // "fast" has few gates, "slow" has many gates exceeding T2.
        // Mock T2 values are ~127-150us for centre qubits.
        let program_depths = vec![
            ("fast".to_string(), 5, 5, 0, 1, ),    // tiny circuit
            ("slow".to_string(), 1000, 0, 1000, 0), // 1000 two-qubit gates = 500us
        ];

        let results =
            analyze_partition_coherence(&plan, &program_depths, &cal, &GateTiming::default());
        assert_eq!(results.len(), 2);

        let fast_result = results.iter().find(|(id, _)| id == "fast").unwrap();
        let slow_result = results.iter().find(|(id, _)| id == "slow").unwrap();

        assert!(fast_result.1.within_coherence);
        assert!(!slow_result.1.within_coherence);
    }

    #[test]
    fn zero_gates_proceeds() {
        let cals = vec![(0, 100.0), (1, 100.0)];
        let result = analyze_coherence(0, 0, 0, 0, &cals, &GateTiming::default());

        assert!(result.within_coherence);
        assert!((result.circuit_time_us).abs() < 1e-9);
        assert_eq!(result.recommendation, CoherenceAction::Proceed);
    }

    #[test]
    fn trapped_ion_timing_different_from_superconducting() {
        // Trapped-ion: much longer gate times, but much longer T2.
        let ion_timing = GateTiming {
            single_qubit_us: 10.0,  // 10us single-qubit
            two_qubit_us: 200.0,    // 200us two-qubit (MS gate)
            measurement_us: 100.0,  // 100us measurement
        };

        // 10 two-qubit gates on trapped ion = 2000us.
        // T2 = 500_000us (0.5s, typical trapped ion). Ratio = 0.004 → Proceed.
        let ion_cals = vec![(0, 500_000.0), (1, 500_000.0)];
        let ion_result = analyze_coherence(10, 0, 10, 0, &ion_cals, &ion_timing);
        assert!(ion_result.within_coherence);
        assert_eq!(ion_result.recommendation, CoherenceAction::Proceed);
        assert!((ion_result.circuit_time_us - 2000.0).abs() < 1e-9);

        // Same circuit on superconducting: 10 two-qubit gates = 5us.
        // T2 = 100us. Ratio = 0.05 → also Proceed, but faster.
        let sc_cals = vec![(0, 100.0), (1, 100.0)];
        let sc_result = analyze_coherence(10, 0, 10, 0, &sc_cals, &GateTiming::default());
        assert!(sc_result.within_coherence);
        assert!((sc_result.circuit_time_us - 5.0).abs() < 1e-9);

        // Trapped ion circuit time is much longer, but T2 ratio is smaller
        // because T2 is proportionally much larger.
        assert!(ion_result.circuit_time_us > sc_result.circuit_time_us);
        assert!(ion_result.t2_ratio < sc_result.t2_ratio);
    }

    #[test]
    fn no_qubits_zero_gates_proceeds() {
        let result = analyze_coherence(0, 0, 0, 0, &[], &GateTiming::default());
        assert!(result.within_coherence);
        assert_eq!(result.recommendation, CoherenceAction::Proceed);
        assert!(result.qubit_margins.is_empty());
    }

    #[test]
    fn measurements_contribute_to_circuit_time() {
        // 5 measurements at 1.0us = 5.0us.
        let cals = vec![(0, 100.0)];
        let result = analyze_coherence(5, 0, 0, 5, &cals, &GateTiming::default());
        assert!((result.circuit_time_us - 5.0).abs() < 1e-9);
    }
}
