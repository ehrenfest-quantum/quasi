// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Multi-program QPU spatial scheduling (Phase C).
//!
//! Multiple programs arrive concurrently. The kernel partitions the QPU into
//! disjoint qubit regions and assigns each program to a region, greedily by
//! priority. High-priority programs get the best qubits.
//!
//! This is the quantum equivalent of multi-core scheduling: a 156-qubit
//! processor running a 20-qubit program wastes 136 qubits. Three 20-qubit
//! programs can run simultaneously on disjoint qubit sets — 3x throughput,
//! same hardware cost.

use std::collections::HashSet;

use crate::calibration::BackendCalibration;
use crate::qubit_map::find_best_region;

/// A request to run a program on the QPU.
#[derive(Debug, Clone)]
pub struct ProgramRequest {
    /// Unique program identifier.
    pub id: String,
    /// Number of qubits needed.
    pub n_qubits: u32,
    /// Priority (higher = more important, gets better qubits).
    pub priority: u32,
}

/// A single program's allocation within a partition.
#[derive(Debug, Clone)]
pub struct ProgramAllocation {
    /// Which program this is for.
    pub program_id: String,
    /// Physical qubits assigned.
    pub physical_qubits: Vec<u32>,
    /// Quality score of the assigned region.
    pub region_score: f64,
}

/// Result of partitioning a QPU for multiple programs.
#[derive(Debug, Clone)]
pub struct PartitionPlan {
    /// Backend this plan is for.
    pub backend_name: String,
    /// Total qubits on the backend.
    pub total_qubits: u32,
    /// Allocations for each accepted program.
    pub allocations: Vec<ProgramAllocation>,
    /// Programs that couldn't fit (rejected due to insufficient qubits).
    pub rejected: Vec<String>,
    /// Qubits left unallocated.
    pub idle_qubits: u32,
    /// Overall utilization (allocated / total).
    pub utilization: f64,
}

/// Partition a QPU for multiple concurrent programs.
///
/// Strategy:
/// 1. Sort programs by priority (highest first)
/// 2. For each program, find the best available region using calibration data
///    (from `qubit_map::find_best_region`), excluding already-allocated qubits
/// 3. If a program can't fit, add to rejected list
/// 4. Return the complete partition plan
///
/// This is greedy by priority — high-priority programs get the best qubits.
/// A future version could use the ATW Hamiltonian to find the globally
/// optimal partition via quantum measurement.
pub fn partition_qpu(
    programs: &[ProgramRequest],
    calibration: &BackendCalibration,
    topology_edges: &[(u32, u32)],
    total_qubits: u32,
) -> PartitionPlan {
    // Sort by priority descending (stable sort to preserve order for equal priority)
    let mut sorted: Vec<&ProgramRequest> = programs.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.priority));

    let mut allocated_qubits: HashSet<u32> = HashSet::new();
    let mut allocations: Vec<ProgramAllocation> = Vec::new();
    let mut rejected: Vec<String> = Vec::new();

    for prog in &sorted {
        if prog.n_qubits == 0 {
            // Zero-qubit programs are trivially allocated.
            allocations.push(ProgramAllocation {
                program_id: prog.id.clone(),
                physical_qubits: vec![],
                region_score: 0.0,
            });
            continue;
        }

        let available_count = total_qubits as usize - allocated_qubits.len();
        if prog.n_qubits as usize > available_count {
            rejected.push(prog.id.clone());
            continue;
        }

        // Filter topology edges to only include available (unallocated) qubits.
        let filtered_edges: Vec<(u32, u32)> = topology_edges
            .iter()
            .filter(|(a, b)| !allocated_qubits.contains(a) && !allocated_qubits.contains(b))
            .copied()
            .collect();

        // Build a filtered calibration with allocated qubits scored at 0.
        let filtered_cal = mask_calibration(calibration, &allocated_qubits);

        // Use the available qubit count as total_qubits for region search,
        // but we need to pass the real total since qubit indices go up to total_qubits.
        let region = find_best_region(prog.n_qubits, &filtered_cal, &filtered_edges, total_qubits);

        // Verify the region doesn't overlap with already-allocated qubits.
        let has_collision = region.mapping.iter().any(|q| allocated_qubits.contains(q));
        if has_collision || region.mapping.len() < prog.n_qubits as usize {
            rejected.push(prog.id.clone());
            continue;
        }

        // Mark qubits as allocated.
        for &q in &region.mapping {
            allocated_qubits.insert(q);
        }

        // Score against the original (unmasked) calibration for honest reporting.
        let score = calibration.region_score(&region.mapping);

        allocations.push(ProgramAllocation {
            program_id: prog.id.clone(),
            physical_qubits: region.mapping,
            region_score: score,
        });
    }

    let allocated_count: usize = allocations.iter().map(|a| a.physical_qubits.len()).sum();
    let idle = total_qubits - allocated_count as u32;
    let utilization = if total_qubits > 0 {
        allocated_count as f64 / total_qubits as f64
    } else {
        0.0
    };

    PartitionPlan {
        backend_name: calibration.backend_name.clone(),
        total_qubits,
        allocations,
        rejected,
        idle_qubits: idle,
        utilization,
    }
}

/// Create a calibration copy where allocated qubits have their scores zeroed out.
///
/// This makes `find_best_region` avoid already-allocated qubits without
/// needing to renumber the qubit indices.
fn mask_calibration(
    calibration: &BackendCalibration,
    allocated: &HashSet<u32>,
) -> BackendCalibration {
    let mut masked = calibration.clone();
    for &q in allocated {
        if let Some(qcal) = masked.qubits.get_mut(q as usize) {
            qcal.t1 = 0.0;
            qcal.t2 = 0.0;
            qcal.gate_fidelity = 0.0;
            qcal.readout_fidelity = 0.0;
        }
    }
    masked
}

/// Check if a partition plan has any qubit collisions.
///
/// Returns `true` if the plan is valid:
/// - All allocated qubits are unique across all programs
/// - All qubits are within bounds
/// - No program appears twice
pub fn validate_partition(plan: &PartitionPlan) -> bool {
    let mut all_qubits: HashSet<u32> = HashSet::new();
    let mut program_ids: HashSet<&str> = HashSet::new();

    for alloc in &plan.allocations {
        // Check for duplicate program IDs.
        if !program_ids.insert(&alloc.program_id) {
            return false;
        }
        for &q in &alloc.physical_qubits {
            // Check bounds.
            if q >= plan.total_qubits {
                return false;
            }
            // Check for qubit collisions across programs.
            if !all_qubits.insert(q) {
                return false;
            }
        }
    }

    true
}

/// Compute throughput improvement: how many programs fit vs sequential execution.
///
/// If 3 programs are allocated concurrently when sequential execution would
/// run them one at a time, the multiplier is 3.0.
pub fn throughput_multiplier(plan: &PartitionPlan, sequential_programs: usize) -> f64 {
    plan.allocations.len() as f64 / sequential_programs.max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::BackendCalibration;

    fn linear_edges(n: u32) -> Vec<(u32, u32)> {
        (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
    }

    fn grid_edges(rows: u32, cols: u32) -> Vec<(u32, u32)> {
        let mut edges = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let q = r * cols + c;
                if c + 1 < cols {
                    edges.push((q, q + 1));
                }
                if r + 1 < rows {
                    edges.push((q, q + cols));
                }
            }
        }
        edges
    }

    #[test]
    fn single_program_fits_entirely() {
        let total = 20u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![ProgramRequest {
            id: "prog1".into(),
            n_qubits: 5,
            priority: 1,
        }];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 1);
        assert_eq!(plan.rejected.len(), 0);
        assert_eq!(plan.allocations[0].program_id, "prog1");
        assert_eq!(plan.allocations[0].physical_qubits.len(), 5);
        assert!(validate_partition(&plan));
    }

    #[test]
    fn two_programs_fit_side_by_side() {
        let total = 50u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "prog_a".into(),
                n_qubits: 10,
                priority: 2,
            },
            ProgramRequest {
                id: "prog_b".into(),
                n_qubits: 10,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 2);
        assert_eq!(plan.rejected.len(), 0);
        assert!(validate_partition(&plan));

        // Utilization: 20 / 50 = 0.4
        let expected_util = 20.0 / 50.0;
        assert!(
            (plan.utilization - expected_util).abs() < 1e-9,
            "expected utilization {expected_util}, got {}",
            plan.utilization,
        );
        assert_eq!(plan.idle_qubits, 30);
    }

    #[test]
    fn three_programs_not_enough_qubits_for_all() {
        let total = 20u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "big1".into(),
                n_qubits: 10,
                priority: 3,
            },
            ProgramRequest {
                id: "big2".into(),
                n_qubits: 10,
                priority: 2,
            },
            ProgramRequest {
                id: "big3".into(),
                n_qubits: 10,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 2);
        assert_eq!(plan.rejected.len(), 1);
        assert_eq!(plan.rejected[0], "big3");
        assert!(validate_partition(&plan));
    }

    #[test]
    fn priority_ordering_high_priority_gets_better_region() {
        let total = 50u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "high_prio".into(),
                n_qubits: 5,
                priority: 10,
            },
            ProgramRequest {
                id: "low_prio".into(),
                n_qubits: 5,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 2);
        assert!(validate_partition(&plan));

        let high_alloc = plan
            .allocations
            .iter()
            .find(|a| a.program_id == "high_prio")
            .unwrap();
        let low_alloc = plan
            .allocations
            .iter()
            .find(|a| a.program_id == "low_prio")
            .unwrap();

        assert!(
            high_alloc.region_score >= low_alloc.region_score,
            "high-priority ({}) should get >= region score than low-priority ({})",
            high_alloc.region_score,
            low_alloc.region_score,
        );
    }

    #[test]
    fn no_qubit_collisions_in_partition() {
        let total = 100u32;
        let edges = grid_edges(10, 10);
        let cal = BackendCalibration::mock("grid", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "p1".into(),
                n_qubits: 15,
                priority: 3,
            },
            ProgramRequest {
                id: "p2".into(),
                n_qubits: 15,
                priority: 2,
            },
            ProgramRequest {
                id: "p3".into(),
                n_qubits: 15,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert!(validate_partition(&plan), "partition must have no collisions");

        // Also verify manually that qubit sets are disjoint.
        let mut all: HashSet<u32> = HashSet::new();
        let mut total_assigned = 0usize;
        for alloc in &plan.allocations {
            for &q in &alloc.physical_qubits {
                all.insert(q);
                total_assigned += 1;
            }
        }
        assert_eq!(
            all.len(),
            total_assigned,
            "all qubits must be unique across programs"
        );
    }

    #[test]
    fn utilization_calculation_correct() {
        let total = 40u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "a".into(),
                n_qubits: 10,
                priority: 2,
            },
            ProgramRequest {
                id: "b".into(),
                n_qubits: 10,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        let expected_util = 20.0 / 40.0;
        assert!(
            (plan.utilization - expected_util).abs() < 1e-9,
            "utilization: expected {expected_util}, got {}",
            plan.utilization,
        );
        assert_eq!(plan.idle_qubits, 20);
    }

    #[test]
    fn empty_program_list_returns_empty_plan() {
        let total = 20u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let plan = partition_qpu(&[], &cal, &edges, total);
        assert!(plan.allocations.is_empty());
        assert!(plan.rejected.is_empty());
        assert_eq!(plan.idle_qubits, total);
        assert_eq!(plan.utilization, 0.0);
        assert!(validate_partition(&plan));
    }

    #[test]
    fn program_requesting_more_qubits_than_backend_is_rejected() {
        let total = 10u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![ProgramRequest {
            id: "too_big".into(),
            n_qubits: 20,
            priority: 1,
        }];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert!(plan.allocations.is_empty());
        assert_eq!(plan.rejected.len(), 1);
        assert_eq!(plan.rejected[0], "too_big");
        assert_eq!(plan.idle_qubits, total);
    }

    #[test]
    fn throughput_multiplier_three_programs() {
        let total = 100u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let programs = vec![
            ProgramRequest {
                id: "p1".into(),
                n_qubits: 10,
                priority: 3,
            },
            ProgramRequest {
                id: "p2".into(),
                n_qubits: 10,
                priority: 2,
            },
            ProgramRequest {
                id: "p3".into(),
                n_qubits: 10,
                priority: 1,
            },
        ];

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 3);
        let mult = throughput_multiplier(&plan, 1);
        assert!(
            (mult - 3.0).abs() < 1e-9,
            "3 concurrent programs vs 1 sequential = 3x, got {mult}"
        );
    }

    #[test]
    fn throughput_multiplier_sequential_zero_handled() {
        let plan = PartitionPlan {
            backend_name: "test".into(),
            total_qubits: 10,
            allocations: vec![],
            rejected: vec![],
            idle_qubits: 10,
            utilization: 0.0,
        };
        let mult = throughput_multiplier(&plan, 0);
        assert_eq!(mult, 0.0);
    }

    #[test]
    fn validate_partition_detects_collision() {
        let plan = PartitionPlan {
            backend_name: "test".into(),
            total_qubits: 10,
            allocations: vec![
                ProgramAllocation {
                    program_id: "a".into(),
                    physical_qubits: vec![0, 1, 2],
                    region_score: 0.9,
                },
                ProgramAllocation {
                    program_id: "b".into(),
                    physical_qubits: vec![2, 3, 4], // qubit 2 collides
                    region_score: 0.8,
                },
            ],
            rejected: vec![],
            idle_qubits: 4,
            utilization: 0.6,
        };
        assert!(!validate_partition(&plan));
    }

    #[test]
    fn validate_partition_detects_out_of_bounds() {
        let plan = PartitionPlan {
            backend_name: "test".into(),
            total_qubits: 10,
            allocations: vec![ProgramAllocation {
                program_id: "a".into(),
                physical_qubits: vec![0, 1, 15], // 15 >= total_qubits
                region_score: 0.9,
            }],
            rejected: vec![],
            idle_qubits: 7,
            utilization: 0.3,
        };
        assert!(!validate_partition(&plan));
    }

    #[test]
    fn validate_partition_detects_duplicate_program() {
        let plan = PartitionPlan {
            backend_name: "test".into(),
            total_qubits: 20,
            allocations: vec![
                ProgramAllocation {
                    program_id: "dup".into(),
                    physical_qubits: vec![0, 1],
                    region_score: 0.9,
                },
                ProgramAllocation {
                    program_id: "dup".into(),
                    physical_qubits: vec![5, 6],
                    region_score: 0.8,
                },
            ],
            rejected: vec![],
            idle_qubits: 16,
            utilization: 0.2,
        };
        assert!(!validate_partition(&plan));
    }

    #[test]
    fn grid_topology_multi_program() {
        // 10x10 grid = 100 qubits, fit 4 x 10-qubit programs
        let total = 100u32;
        let edges = grid_edges(10, 10);
        let cal = BackendCalibration::mock("grid_100", total, &edges);

        let programs: Vec<ProgramRequest> = (0..4)
            .map(|i| ProgramRequest {
                id: format!("grid_prog_{i}"),
                n_qubits: 10,
                priority: 4 - i,
            })
            .collect();

        let plan = partition_qpu(&programs, &cal, &edges, total);
        assert_eq!(plan.allocations.len(), 4);
        assert!(plan.rejected.is_empty());
        assert!(validate_partition(&plan));
        assert_eq!(plan.idle_qubits, 60);
        assert!((plan.utilization - 0.4).abs() < 1e-9);
    }
}
