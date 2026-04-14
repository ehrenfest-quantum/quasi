// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Error budget management (Phase F).
//!
//! Tracks cumulative gate errors per program and decides when to intervene.
//! The kernel checks the error budget before dispatch: if the estimated
//! total error exceeds policy thresholds, it warns or halts with actionable
//! suggestions (re-route, reduce depth, apply mitigation).

/// Error budget tracker for a running program.
#[derive(Debug, Clone)]
pub struct ErrorBudget {
    /// Program identifier.
    pub program_id: String,
    /// Accumulated single-qubit gate error (1 - product of 1Q fidelities).
    pub single_qubit_error: f64,
    /// Accumulated two-qubit gate error (1 - product of 2Q fidelities).
    pub two_qubit_error: f64,
    /// Accumulated readout error (1 - product of readout fidelities).
    pub readout_error: f64,
    /// Total estimated error (1 - product of all fidelities).
    pub total_error: f64,
    /// Number of gates applied.
    pub gate_count: u32,
}

/// Policy for error budget intervention.
#[derive(Debug, Clone)]
pub struct ErrorPolicy {
    /// Warn when total error exceeds this threshold.
    pub warn_threshold: f64,
    /// Halt when total error exceeds this threshold.
    pub halt_threshold: f64,
}

impl Default for ErrorPolicy {
    fn default() -> Self {
        Self {
            warn_threshold: 0.05,
            halt_threshold: 0.10,
        }
    }
}

/// Result of error budget evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorVerdict {
    /// Error within acceptable limits.
    Ok { total_error: f64 },
    /// Error approaching threshold — warn.
    Warn { total_error: f64, threshold: f64 },
    /// Error exceeds threshold — halt or re-route.
    Halt {
        total_error: f64,
        threshold: f64,
        suggestion: ErrorSuggestion,
    },
}

/// Suggestion for how to handle a budget overrun.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSuggestion {
    /// Re-run on a higher-fidelity backend.
    RerouteToHigherFidelity { min_gate_fidelity: f64 },
    /// Reduce circuit depth (fewer Trotter steps, lower precision).
    ReduceDepth { max_gates: u32 },
    /// Use error mitigation (if supported by backend).
    ApplyMitigation,
}

/// Estimate the error budget for a circuit before execution.
///
/// Computes the expected total error from gate counts and per-qubit
/// calibration fidelities. This is what the kernel checks before dispatch.
///
/// The model: total fidelity = (1q_fid)^n_1q * (2q_fid)^n_2q * (ro_fid)^n_meas,
/// total error = 1 - total fidelity.
pub fn estimate_error(
    n_single_qubit_gates: u32,
    n_two_qubit_gates: u32,
    n_measurements: u32,
    avg_1q_fidelity: f64,
    avg_2q_fidelity: f64,
    avg_readout_fidelity: f64,
) -> ErrorBudget {
    let fid_1q = avg_1q_fidelity.powi(n_single_qubit_gates as i32);
    let fid_2q = avg_2q_fidelity.powi(n_two_qubit_gates as i32);
    let fid_ro = avg_readout_fidelity.powi(n_measurements as i32);

    let single_qubit_error = 1.0 - fid_1q;
    let two_qubit_error = 1.0 - fid_2q;
    let readout_error = 1.0 - fid_ro;
    let total_fidelity = fid_1q * fid_2q * fid_ro;
    let total_error = 1.0 - total_fidelity;

    ErrorBudget {
        program_id: String::new(),
        single_qubit_error,
        two_qubit_error,
        readout_error,
        total_error,
        gate_count: n_single_qubit_gates + n_two_qubit_gates + n_measurements,
    }
}

/// Evaluate an error budget against policy.
///
/// Returns `Ok` if within warn threshold, `Warn` if between warn and halt,
/// or `Halt` with a suggestion based on the dominant error source.
pub fn evaluate(budget: &ErrorBudget, policy: &ErrorPolicy) -> ErrorVerdict {
    if budget.total_error >= policy.halt_threshold {
        let suggestion = if budget.two_qubit_error > budget.single_qubit_error
            && budget.two_qubit_error > budget.readout_error
        {
            // Two-qubit errors dominate — need a better backend.
            ErrorSuggestion::RerouteToHigherFidelity {
                min_gate_fidelity: 1.0 - (1.0 - budget.two_qubit_error) * 0.5,
            }
        } else if budget.gate_count > 50 {
            // Many gates — reduce depth.
            ErrorSuggestion::ReduceDepth {
                max_gates: (budget.gate_count as f64
                    * (policy.halt_threshold / budget.total_error))
                    as u32,
            }
        } else {
            ErrorSuggestion::ApplyMitigation
        };

        ErrorVerdict::Halt {
            total_error: budget.total_error,
            threshold: policy.halt_threshold,
            suggestion,
        }
    } else if budget.total_error >= policy.warn_threshold {
        ErrorVerdict::Warn {
            total_error: budget.total_error,
            threshold: policy.warn_threshold,
        }
    } else {
        ErrorVerdict::Ok {
            total_error: budget.total_error,
        }
    }
}

/// Estimate the maximum circuit depth (gate count) achievable
/// within a given error budget on a given backend.
///
/// Solves: target_fidelity = (weighted_avg_fidelity)^n for n,
/// where weighted_avg_fidelity blends 1Q and 2Q fidelities by the
/// two-qubit fraction.
///
/// Returns 0 if the fidelity is too low (weighted average <= target).
pub fn max_feasible_depth(
    target_fidelity: f64,
    avg_1q_fidelity: f64,
    avg_2q_fidelity: f64,
    two_qubit_fraction: f64,
) -> u32 {
    let one_q_fraction = 1.0 - two_qubit_fraction;
    let weighted_fidelity =
        avg_1q_fidelity * one_q_fraction + avg_2q_fidelity * two_qubit_fraction;

    if weighted_fidelity <= 0.0 || weighted_fidelity >= 1.0 && target_fidelity >= 1.0 {
        return 0;
    }
    if weighted_fidelity >= 1.0 {
        // Perfect fidelity — infinite depth.
        return u32::MAX;
    }

    let n = target_fidelity.ln() / weighted_fidelity.ln();
    if n <= 0.0 || n.is_nan() || n.is_infinite() {
        return 0;
    }
    n.floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_gates_zero_error() {
        let budget = estimate_error(0, 0, 0, 0.999, 0.99, 0.98);
        assert!((budget.total_error).abs() < 1e-12);
        assert!((budget.single_qubit_error).abs() < 1e-12);
        assert!((budget.two_qubit_error).abs() < 1e-12);
        assert!((budget.readout_error).abs() < 1e-12);
        assert_eq!(budget.gate_count, 0);
    }

    #[test]
    fn hundred_1q_gates_at_0999_fidelity() {
        // 0.999^100 ≈ 0.9048, so error ≈ 0.0952
        let budget = estimate_error(100, 0, 0, 0.999, 1.0, 1.0);
        let expected = 1.0 - 0.999_f64.powi(100);
        assert!(
            (budget.total_error - expected).abs() < 1e-9,
            "got {} expected {}",
            budget.total_error,
            expected
        );
        assert!(budget.total_error > 0.09 && budget.total_error < 0.10);
    }

    #[test]
    fn low_gate_count_high_fidelity_ok() {
        let budget = estimate_error(10, 2, 1, 0.999, 0.99, 0.98);
        let policy = ErrorPolicy::default();
        let verdict = evaluate(&budget, &policy);
        match verdict {
            ErrorVerdict::Ok { total_error } => {
                assert!(total_error < policy.warn_threshold);
            }
            other => panic!("expected Ok, got {:?}", other),
        }
    }

    #[test]
    fn medium_gates_warn() {
        // 50 1Q gates at 0.999: error ≈ 0.0488 → just under 5% warn
        // 60 1Q gates at 0.999: error ≈ 0.0583 → above 5% warn, below 10% halt
        let budget = estimate_error(60, 0, 0, 0.999, 1.0, 1.0);
        let policy = ErrorPolicy::default();
        let verdict = evaluate(&budget, &policy);
        match verdict {
            ErrorVerdict::Warn {
                total_error,
                threshold,
            } => {
                assert!(total_error >= 0.05);
                assert!(total_error < 0.10);
                assert!((threshold - 0.05).abs() < 1e-12);
            }
            other => panic!("expected Warn, got {:?}", other),
        }
    }

    #[test]
    fn high_gate_count_halt_with_suggestion() {
        // 200 1Q gates at 0.999: error ≈ 0.1814 → above 10% halt
        let budget = estimate_error(200, 0, 0, 0.999, 1.0, 1.0);
        let policy = ErrorPolicy::default();
        let verdict = evaluate(&budget, &policy);
        match verdict {
            ErrorVerdict::Halt {
                total_error,
                threshold,
                suggestion,
            } => {
                assert!(total_error > 0.10);
                assert!((threshold - 0.10).abs() < 1e-12);
                // 200 gates, 1Q error dominates, gate_count > 50 → ReduceDepth
                match suggestion {
                    ErrorSuggestion::ReduceDepth { max_gates } => {
                        assert!(max_gates < 200);
                    }
                    other => panic!("expected ReduceDepth, got {:?}", other),
                }
            }
            other => panic!("expected Halt, got {:?}", other),
        }
    }

    #[test]
    fn two_qubit_error_dominates_reroute() {
        // Few 1Q gates but many 2Q gates with low fidelity → 2Q error dominates
        let budget = estimate_error(5, 20, 0, 0.999, 0.98, 1.0);
        let policy = ErrorPolicy {
            warn_threshold: 0.05,
            halt_threshold: 0.10,
        };
        // 0.98^20 ≈ 0.6676, 2Q error ≈ 0.3324
        // 0.999^5 ≈ 0.995, 1Q error ≈ 0.005
        // Total error ≈ 1 - 0.6676 * 0.995 ≈ 0.336
        let verdict = evaluate(&budget, &policy);
        match verdict {
            ErrorVerdict::Halt { suggestion, .. } => match suggestion {
                ErrorSuggestion::RerouteToHigherFidelity { .. } => {}
                other => panic!("expected RerouteToHigherFidelity, got {:?}", other),
            },
            other => panic!("expected Halt, got {:?}", other),
        }
    }

    #[test]
    fn tight_policy_triggers_earlier() {
        let budget = estimate_error(20, 0, 0, 0.999, 1.0, 1.0);
        // error ≈ 0.0198

        let relaxed = ErrorPolicy::default(); // warn 0.05, halt 0.10
        let tight = ErrorPolicy {
            warn_threshold: 0.01,
            halt_threshold: 0.02,
        };

        let relaxed_verdict = evaluate(&budget, &relaxed);
        let tight_verdict = evaluate(&budget, &tight);

        assert!(matches!(relaxed_verdict, ErrorVerdict::Ok { .. }));
        assert!(matches!(tight_verdict, ErrorVerdict::Warn { .. }));
    }

    #[test]
    fn default_policy_thresholds() {
        let policy = ErrorPolicy::default();
        assert!((policy.warn_threshold - 0.05).abs() < 1e-12);
        assert!((policy.halt_threshold - 0.10).abs() < 1e-12);
    }

    #[test]
    fn max_feasible_depth_099_target_0999_fidelity() {
        // log(0.99) / log(0.999) ≈ 10.04 → floor = 10
        let depth = max_feasible_depth(0.99, 0.999, 0.999, 0.0);
        assert_eq!(depth, 10);
    }

    #[test]
    fn max_feasible_depth_090_target_0999_fidelity() {
        // log(0.90) / log(0.999) ≈ 105.36 → floor = 105
        let depth = max_feasible_depth(0.90, 0.999, 0.999, 0.0);
        assert!(
            depth >= 100 && depth <= 110,
            "expected ~105, got {}",
            depth
        );
    }

    #[test]
    fn max_feasible_depth_with_two_qubit_fraction() {
        // 50% 2Q gates: weighted_fid = 0.5*0.999 + 0.5*0.99 = 0.9945
        // log(0.90) / log(0.9945) ≈ 19.1 → floor = 19
        let depth = max_feasible_depth(0.90, 0.999, 0.99, 0.5);
        assert!(depth > 15 && depth < 25, "expected ~19, got {}", depth);
    }

    #[test]
    fn max_feasible_depth_perfect_fidelity() {
        let depth = max_feasible_depth(0.99, 1.0, 1.0, 0.5);
        assert_eq!(depth, u32::MAX);
    }
}
