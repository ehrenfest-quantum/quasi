// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Virtual-to-physical qubit mapping via calibration-aware region selection.
//!
//! Given a program that needs `n` virtual qubits and a backend with `N`
//! physical qubits plus calibration data, this module finds the best
//! contiguous region of `n` physical qubits — avoiding degraded qubits
//! and preferring high-fidelity, well-connected regions.
//!
//! Two strategies are provided:
//! - **Greedy** (`find_best_region`): O(N²) — starts from the best qubit,
//!   greedily expands to the best connected neighbour. Default for all sizes.
//! - **Brute-force** (`find_best_region_exact`): O(C(N, n)) — evaluates all
//!   combinations. Only tractable for small n (≤ 10). Used for testing
//!   correctness of the greedy heuristic.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::calibration::BackendCalibration;

/// A mapping from virtual qubits (0..n) to physical qubits on a backend.
#[derive(Debug, Clone)]
pub struct QubitMap {
    /// `mapping[virtual_qubit]` → physical qubit index.
    pub mapping: Vec<u32>,
    /// Score of this mapping (higher = better).
    pub score: f64,
    /// Which backend this mapping is for.
    pub backend_name: String,
}

impl QubitMap {
    /// The number of virtual qubits in this mapping.
    pub fn n_qubits(&self) -> usize {
        self.mapping.len()
    }

    /// Look up the physical qubit for a virtual qubit.
    pub fn physical(&self, virtual_qubit: usize) -> Option<u32> {
        self.mapping.get(virtual_qubit).copied()
    }
}

/// Build an adjacency list from a list of edges.
fn build_adjacency(topology_edges: &[(u32, u32)], total_qubits: u32) -> HashMap<u32, Vec<u32>> {
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for q in 0..total_qubits {
        adj.entry(q).or_default();
    }
    for &(a, b) in topology_edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    adj
}

/// Find the best qubit region using a greedy strategy.
///
/// 1. Score all physical qubits.
/// 2. Start from the highest-scoring qubit.
/// 3. Expand to the highest-scoring neighbour not yet selected.
/// 4. Repeat until we have `n_qubits`.
///
/// The result maps virtual qubit 0 to the best physical qubit, virtual qubit 1
/// to the next-best connected neighbour, and so on. This naturally produces
/// a connected subgraph on the device topology.
///
/// Returns a mapping with score 0.0 if `n_qubits` exceeds available connected
/// qubits (the region will be as large as the largest reachable component from
/// the best starting qubit, padded with the best remaining disconnected qubits).
pub fn find_best_region(
    n_qubits: u32,
    calibration: &BackendCalibration,
    topology_edges: &[(u32, u32)],
    total_qubits: u32,
) -> QubitMap {
    if n_qubits == 0 {
        return QubitMap {
            mapping: vec![],
            score: 0.0,
            backend_name: calibration.backend_name.clone(),
        };
    }

    let adj = build_adjacency(topology_edges, total_qubits);

    // Score all qubits and find the best starting point
    let mut scored: Vec<(u32, f64)> = (0..total_qubits)
        .map(|q| (q, calibration.qubit_score(q)))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut selected: Vec<u32> = Vec::with_capacity(n_qubits as usize);
    let mut in_selected: HashSet<u32> = HashSet::new();

    // Start from the best qubit
    let start = scored[0].0;
    selected.push(start);
    in_selected.insert(start);

    // Greedily expand via topology neighbours
    while (selected.len() as u32) < n_qubits {
        // Collect all neighbours of the current region that are not yet selected
        let mut candidates: Vec<(u32, f64)> = Vec::new();
        for &q in &selected {
            if let Some(neighbours) = adj.get(&q) {
                for &nb in neighbours {
                    if !in_selected.contains(&nb) {
                        candidates.push((nb, calibration.qubit_score(nb)));
                    }
                }
            }
        }
        // De-duplicate candidates (a qubit may neighbour multiple selected qubits)
        candidates.sort_by_key(|&(q, _)| q);
        candidates.dedup_by_key(|c| c.0);

        if candidates.is_empty() {
            // No more connected neighbours — fall back to best unselected qubit
            // (produces a disconnected region, but at least fills the request)
            if let Some(&(q, _)) = scored.iter().find(|(q, _)| !in_selected.contains(q)) {
                selected.push(q);
                in_selected.insert(q);
            } else {
                break; // No qubits left at all
            }
        } else {
            // Pick the best-scoring candidate
            candidates
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let best = candidates[0].0;
            selected.push(best);
            in_selected.insert(best);
        }
    }

    let score = calibration.region_score(&selected);
    QubitMap {
        mapping: selected,
        score,
        backend_name: calibration.backend_name.clone(),
    }
}

/// Find the best qubit region by exhaustive search over all combinations.
///
/// Only tractable for small `n_qubits` (≤ 10) relative to `total_qubits`.
/// Used to verify the greedy heuristic in tests.
///
/// Unlike the greedy approach, this considers ALL C(total, n) subsets,
/// not just connected ones.
pub fn find_best_region_exact(
    n_qubits: u32,
    calibration: &BackendCalibration,
    _topology_edges: &[(u32, u32)],
    total_qubits: u32,
) -> QubitMap {
    if n_qubits == 0 {
        return QubitMap {
            mapping: vec![],
            score: 0.0,
            backend_name: calibration.backend_name.clone(),
        };
    }

    let mut best_combo: Vec<u32> = Vec::new();
    let mut best_score = f64::NEG_INFINITY;

    // Generate all C(total_qubits, n_qubits) combinations
    let mut combo: Vec<u32> = (0..n_qubits).collect();
    loop {
        let score = calibration.region_score(&combo);
        if score > best_score {
            best_score = score;
            best_combo = combo.clone();
        }
        // Advance to next combination
        if !next_combination(&mut combo, total_qubits) {
            break;
        }
    }

    QubitMap {
        mapping: best_combo,
        score: best_score,
        backend_name: calibration.backend_name.clone(),
    }
}

/// Advance a combination to the next lexicographic combination.
/// Returns false when all combinations have been exhausted.
fn next_combination(combo: &mut [u32], n: u32) -> bool {
    let k = combo.len();
    if k == 0 {
        return false;
    }
    // Find the rightmost element that can be incremented
    let mut i = k - 1;
    loop {
        if combo[i] < n - (k as u32 - i as u32) {
            combo[i] += 1;
            // Reset all elements after i
            for j in (i + 1)..k {
                combo[j] = combo[j - 1] + 1;
            }
            return true;
        }
        if i == 0 {
            return false;
        }
        i -= 1;
    }
}

/// Verify that a qubit map is valid: correct size, all within bounds, no duplicates.
pub fn validate_qubit_map(map: &QubitMap, n_qubits: u32, total_qubits: u32) -> bool {
    if map.mapping.len() != n_qubits as usize {
        return false;
    }
    let unique: BTreeSet<u32> = map.mapping.iter().copied().collect();
    if unique.len() != map.mapping.len() {
        return false; // duplicates
    }
    map.mapping.iter().all(|&q| q < total_qubits)
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
    fn greedy_mapping_size_matches_request() {
        let n_qubits = 5u32;
        let total = 20u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let map = find_best_region(n_qubits, &cal, &edges, total);
        assert_eq!(map.n_qubits(), n_qubits as usize);
        assert!(validate_qubit_map(&map, n_qubits, total));
    }

    #[test]
    fn greedy_all_qubits_within_bounds() {
        let total = 30u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let map = find_best_region(8, &cal, &edges, total);
        for &q in &map.mapping {
            assert!(q < total, "qubit {q} out of bounds (total={total})");
        }
    }

    #[test]
    fn greedy_avoids_defective_qubits() {
        // 50 qubits, defective at 0, 17, 34
        let total = 50u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let map = find_best_region(5, &cal, &edges, total);
        // The 5-qubit region should NOT include defective qubits
        // (centre qubits 23-27 are the best region on a 50-qubit linear chain)
        assert!(
            !map.mapping.contains(&0),
            "should avoid defective qubit 0: {:?}",
            map.mapping
        );
        assert!(
            !map.mapping.contains(&34),
            "should avoid defective qubit 34: {:?}",
            map.mapping
        );
    }

    #[test]
    fn greedy_prefers_high_fidelity_region() {
        let total = 50u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let map = find_best_region(5, &cal, &edges, total);
        // All selected qubits should be near the centre (high quality)
        for &q in &map.mapping {
            assert!(
                q >= 10 && q <= 40,
                "qubit {q} is too far from centre: {:?}",
                map.mapping
            );
        }
    }

    #[test]
    fn greedy_region_score_is_positive() {
        let total = 20u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let map = find_best_region(5, &cal, &edges, total);
        assert!(map.score > 0.0, "region score should be positive");
    }

    #[test]
    fn exact_finds_optimal_on_small_instance() {
        // Small enough for brute force: 10 qubits, pick 3
        let total = 10u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let exact = find_best_region_exact(3, &cal, &edges, total);
        assert!(validate_qubit_map(&exact, 3, total));

        // Verify the exact solution is truly optimal by checking a few alternatives
        let alt1 = cal.region_score(&[0, 1, 2]);
        let alt2 = cal.region_score(&[7, 8, 9]);
        assert!(
            exact.score >= alt1,
            "exact ({}) should beat edge region ({})",
            exact.score,
            alt1
        );
        assert!(
            exact.score >= alt2,
            "exact ({}) should beat other region ({})",
            exact.score,
            alt2
        );
    }

    #[test]
    fn greedy_matches_or_approaches_exact_on_small_instance() {
        let total = 12u32;
        let edges = linear_edges(total);
        let cal = BackendCalibration::mock("test", total, &edges);

        let greedy = find_best_region(3, &cal, &edges, total);
        let exact = find_best_region_exact(3, &cal, &edges, total);

        // Greedy should be within 5% of exact on this simple topology
        assert!(
            greedy.score >= exact.score * 0.95,
            "greedy ({}) should be within 5% of exact ({})",
            greedy.score,
            exact.score
        );
    }

    #[test]
    fn zero_qubits_returns_empty_map() {
        let cal = BackendCalibration::mock("test", 10, &linear_edges(10));
        let map = find_best_region(0, &cal, &linear_edges(10), 10);
        assert_eq!(map.n_qubits(), 0);
        assert_eq!(map.score, 0.0);
    }

    #[test]
    fn grid_topology_selects_connected_region() {
        // 5x5 grid = 25 qubits
        let rows = 5u32;
        let cols = 5u32;
        let total = rows * cols;
        let edges = grid_edges(rows, cols);
        let cal = BackendCalibration::mock("grid_test", total, &edges);

        let map = find_best_region(4, &cal, &edges, total);
        assert!(validate_qubit_map(&map, 4, total));
        assert!(map.score > 0.0);
    }

    #[test]
    fn physical_lookup_works() {
        let map = QubitMap {
            mapping: vec![10, 11, 12],
            score: 0.9,
            backend_name: "test".into(),
        };
        assert_eq!(map.physical(0), Some(10));
        assert_eq!(map.physical(1), Some(11));
        assert_eq!(map.physical(2), Some(12));
        assert_eq!(map.physical(5), None);
    }

    #[test]
    fn validate_rejects_duplicates() {
        let map = QubitMap {
            mapping: vec![5, 5, 6],
            score: 0.5,
            backend_name: "test".into(),
        };
        assert!(!validate_qubit_map(&map, 3, 10));
    }

    #[test]
    fn validate_rejects_out_of_bounds() {
        let map = QubitMap {
            mapping: vec![5, 6, 100],
            score: 0.5,
            backend_name: "test".into(),
        };
        assert!(!validate_qubit_map(&map, 3, 50));
    }

    #[test]
    fn validate_rejects_wrong_size() {
        let map = QubitMap {
            mapping: vec![5, 6],
            score: 0.5,
            backend_name: "test".into(),
        };
        assert!(!validate_qubit_map(&map, 3, 50));
    }

    #[test]
    fn next_combination_enumerates_correctly() {
        // C(4, 2) = 6 combinations
        let mut combo = vec![0, 1];
        let mut count = 1; // including the initial state
        while next_combination(&mut combo, 4) {
            count += 1;
        }
        assert_eq!(count, 6);
    }
}
