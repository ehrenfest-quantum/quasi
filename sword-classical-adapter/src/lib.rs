// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! SWORD Classical Optimization Adapter (3c).
//!
//! Translates SWORD documents (emitted by Arn's `/analyze?output=sword`)
//! into formats that classical solvers consume, for cases where
//! `recommended_path = "classical_optimization"` or `"black_box"`.
//!
//! Only volatile islands are translated — stable parts are already solved.
//! Local island node indices are mapped to global indices via the graph.
//!
//! # Output formats
//! - [`QuboMatrix`]   — dense QUBO matrix for SA, Gurobi, D-Wave
//! - [`IsingModel`]   — `{h, J}` vectors for SA, kicked-Ising solvers
//! - [`RoutingSummary`] — segment lists + adjacency for VRP/distance problems
//!
//! # Garm API
//! [`sword_to_garm_request`] formats the volatile QUBO as a
//! `POST /solve_qubo` body: `{ "matrix": [[...]], "shots": N }`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("SWORD document missing field: {0}")]
    MissingField(&'static str),
    #[error("SWORD document has no volatile islands")]
    NoVolatileIslands,
    #[error("volatile islands contain no nodes")]
    EmptyIslands,
    #[error("unexpected JSON type for field: {0}")]
    TypeMismatch(&'static str),
}

type Result<T> = std::result::Result<T, AdapterError>;

// ── Output types ─────────────────────────────────────────────────────────────

/// Dense QUBO matrix for SA, Gurobi, and D-Wave samplers.
///
/// Q is upper-triangular by convention: Q[i][j] with i ≤ j.
/// Diagonal Q[i][i] encodes linear bias (h_i), off-diagonal encodes coupling
/// (j_ij). The QUBO objective is x^T Q x with x ∈ {0,1}.
///
/// Note: SWORD uses Ising variables ∈ {-1, +1}. The conversion from Ising
/// (h, J) to QUBO Q is: Q[i][i] = 2*h_i, Q[i][j] = 4*J_ij (i < j).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuboMatrix {
    /// Number of binary variables (= total volatile nodes across all islands).
    pub n: usize,
    /// Flat upper-triangular QUBO matrix, row-major, length n*n.
    /// Entry [i*n + j] for i ≤ j, 0 elsewhere.
    pub matrix: Vec<Vec<f64>>,
    /// Mapping from matrix row/col index to global node id.
    pub index_to_node: Vec<u64>,
}

/// Ising model for SA solvers and kicked-Ising hardware.
///
/// H = Σ_i h[i] Z_i + Σ_{(i,j)} J[(i,j)] Z_i Z_j
/// with Z ∈ {-1, +1}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsingModel {
    /// Local fields h_i indexed by local (compact) index.
    pub h: Vec<f64>,
    /// Coupling map: key (i, j) with i < j, value J_ij.
    pub j: HashMap<(usize, usize), f64>,
    /// Mapping from local index to global node id.
    pub index_to_node: Vec<u64>,
}

/// Routing summary for distance/VRP problems
/// (`extractor = "distance"` in SWORD provenance).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSummary {
    /// Volatile segments: node pairs that need route optimization.
    pub volatile_segments: Vec<[u64; 2]>,
    /// Fixed/stable segments: already-decided arcs.
    pub fixed_segments: Vec<[u64; 2]>,
    /// Adjacency list of volatile nodes (global ids).
    pub adjacency: HashMap<u64, Vec<u64>>,
}

// ── Default shots for Garm requests ─────────────────────────────────────────

const DEFAULT_SHOTS: u32 = 1024;

// ── Helper: extract volatile node set and sorted index ──────────────────────

/// Parse the graph nodes/edges and volatile islands from a SWORD document.
/// Returns (node_h, edge_j, sorted_volatile_ids, stable_edges_array).
fn parse_sword(sword: &Value) -> Result<(
    HashMap<u64, f64>,
    HashMap<(u64, u64), f64>,
    Vec<u64>,
    Vec<Value>,
)> {
    let partition = sword
        .get("partition")
        .ok_or(AdapterError::MissingField("partition"))?;

    let graph = partition
        .get("graph")
        .ok_or(AdapterError::MissingField("partition.graph"))?;

    // ── Parse nodes ──────────────────────────────────────────────────────
    let nodes = graph
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or(AdapterError::MissingField("graph.nodes"))?;

    let mut node_h: HashMap<u64, f64> = HashMap::new();
    for node in nodes {
        let id = node
            .get("id")
            .and_then(|v| v.as_u64())
            .ok_or(AdapterError::TypeMismatch("node.id"))?;
        let h_i = node.get("h_i").and_then(|v| v.as_f64()).unwrap_or(0.0);
        node_h.insert(id, h_i);
    }

    // ── Parse edges ──────────────────────────────────────────────────────
    let edges = graph
        .get("edges")
        .and_then(|v| v.as_array())
        .ok_or(AdapterError::MissingField("graph.edges"))?;

    let mut edge_j: HashMap<(u64, u64), f64> = HashMap::new();
    for edge in edges {
        let i = edge
            .get("i")
            .and_then(|v| v.as_u64())
            .ok_or(AdapterError::TypeMismatch("edge.i"))?;
        let j = edge
            .get("j")
            .and_then(|v| v.as_u64())
            .ok_or(AdapterError::TypeMismatch("edge.j"))?;
        let j_ij = edge.get("j_ij").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let key = if i <= j { (i, j) } else { (j, i) };
        edge_j.insert(key, j_ij);
    }

    // ── Collect volatile node ids ─────────────────────────────────────────
    let volatile_islands = partition
        .get("volatile_islands")
        .and_then(|v| v.as_array())
        .ok_or(AdapterError::MissingField("partition.volatile_islands"))?;

    if volatile_islands.is_empty() {
        return Err(AdapterError::NoVolatileIslands);
    }

    let mut volatile_set: HashSet<u64> = HashSet::new();
    for island in volatile_islands {
        if let Some(nids) = island.get("node_ids").and_then(|v| v.as_array()) {
            for nid in nids {
                if let Some(id) = nid.as_u64() {
                    volatile_set.insert(id);
                }
            }
        }
    }

    if volatile_set.is_empty() {
        return Err(AdapterError::EmptyIslands);
    }

    let mut sorted_volatile: Vec<u64> = volatile_set.into_iter().collect();
    sorted_volatile.sort_unstable();

    let stable_edges = partition
        .get("stable_edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok((node_h, edge_j, sorted_volatile, stable_edges))
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Convert a SWORD document into a dense QUBO matrix.
///
/// Only volatile islands are included. The mapping from Ising (h_i, J_ij)
/// to QUBO is: Q[i][i] = 2*h_i, Q[i][j] = 4*J_ij for i < j.
///
/// Accepts any `recommended_path` — the caller is responsible for routing
/// only classical/black_box cases here.
pub fn sword_to_qubo(sword: &Value) -> Result<QuboMatrix> {
    let (node_h, edge_j, sorted_volatile, _stable_edges) = parse_sword(sword)?;

    let n = sorted_volatile.len();
    // Map global node id → local compact index.
    let node_to_idx: HashMap<u64, usize> = sorted_volatile
        .iter()
        .enumerate()
        .map(|(i, &nid)| (nid, i))
        .collect();

    // Initialise n×n matrix with zeros.
    let mut matrix = vec![vec![0.0f64; n]; n];

    // Diagonal: h_i → Q[i][i] = 2 * h_i  (Ising-to-QUBO conversion).
    for (idx, &nid) in sorted_volatile.iter().enumerate() {
        let h = node_h.get(&nid).copied().unwrap_or(0.0);
        matrix[idx][idx] = 2.0 * h;
    }

    // Off-diagonal: J_ij → Q[i][j] = 4 * J_ij  (i < j, upper-triangular).
    for (&(ni, nj), &j_ij) in &edge_j {
        let Some(&li) = node_to_idx.get(&ni) else { continue };
        let Some(&lj) = node_to_idx.get(&nj) else { continue };
        // Enforce upper-triangular storage.
        let (row, col) = if li <= lj { (li, lj) } else { (lj, li) };
        matrix[row][col] += 4.0 * j_ij;
    }

    Ok(QuboMatrix {
        n,
        matrix,
        index_to_node: sorted_volatile,
    })
}

/// Convert a SWORD document into an Ising model (h, J).
///
/// h[i] is the local field for the i-th volatile node (sorted by global id).
/// J[(i,j)] is the coupling between volatile nodes i and j (i < j).
/// Stable boundary nodes are included as mean-field shifts on their volatile
/// neighbours: h[i] += J_ij for each stable neighbour j.
pub fn sword_to_ising(sword: &Value) -> Result<IsingModel> {
    let (node_h, edge_j, sorted_volatile, _stable_edges) = parse_sword(sword)?;

    let n = sorted_volatile.len();
    let node_to_idx: HashMap<u64, usize> = sorted_volatile
        .iter()
        .enumerate()
        .map(|(i, &nid)| (nid, i))
        .collect();

    let mut h = vec![0.0f64; n];
    let mut j_map: HashMap<(usize, usize), f64> = HashMap::new();

    // Local fields from h_i.
    for (idx, &nid) in sorted_volatile.iter().enumerate() {
        h[idx] = node_h.get(&nid).copied().unwrap_or(0.0);
    }

    // Couplings and mean-field boundary terms.
    for (&(ni, nj), &j_ij) in &edge_j {
        if j_ij.abs() < 1e-15 {
            continue;
        }
        let li = node_to_idx.get(&ni);
        let lj = node_to_idx.get(&nj);
        match (li, lj) {
            (Some(&ai), Some(&aj)) => {
                // Both volatile — coupling term.
                let key = if ai < aj { (ai, aj) } else { (aj, ai) };
                *j_map.entry(key).or_insert(0.0) += j_ij;
            }
            (Some(&ai), None) => {
                // nj is stable, ⟨Z_nj⟩ = +1 → contributes J_ij * Z_ni.
                h[ai] += j_ij;
            }
            (None, Some(&aj)) => {
                // ni is stable, ⟨Z_ni⟩ = +1 → contributes J_ij * Z_nj.
                h[aj] += j_ij;
            }
            (None, None) => {} // both stable, already resolved
        }
    }

    Ok(IsingModel {
        h,
        j: j_map,
        index_to_node: sorted_volatile,
    })
}

/// Build a routing summary for distance/VRP problems.
///
/// Volatile island edges become `volatile_segments` requiring optimisation.
/// `stable_edges` from the partition become `fixed_segments`.
/// An adjacency list of volatile nodes is provided for solver convenience.
pub fn sword_to_routing(sword: &Value) -> Result<RoutingSummary> {
    let (_, edge_j, sorted_volatile, stable_edges) = parse_sword(sword)?;

    let volatile_set: HashSet<u64> = sorted_volatile.iter().copied().collect();

    let mut volatile_segments: Vec<[u64; 2]> = Vec::new();
    let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();

    for (&(ni, nj), _) in &edge_j {
        if volatile_set.contains(&ni) && volatile_set.contains(&nj) {
            volatile_segments.push([ni, nj]);
            adjacency.entry(ni).or_default().push(nj);
            adjacency.entry(nj).or_default().push(ni);
        }
    }

    let mut fixed_segments: Vec<[u64; 2]> = Vec::new();
    for se in &stable_edges {
        let i = se.get("i").and_then(|v| v.as_u64());
        let j = se.get("j").and_then(|v| v.as_u64());
        if let (Some(ni), Some(nj)) = (i, j) {
            fixed_segments.push([ni, nj]);
        }
    }

    Ok(RoutingSummary {
        volatile_segments,
        fixed_segments,
        adjacency,
    })
}

/// Format volatile QUBO as a Garm `POST /solve_qubo` request body.
///
/// Shape: `{ "matrix": [[...]], "shots": N }` where N defaults to 1024.
pub fn sword_to_garm_request(sword: &Value) -> Result<Value> {
    sword_to_garm_request_with_shots(sword, DEFAULT_SHOTS)
}

/// Same as [`sword_to_garm_request`] but with an explicit shot count.
pub fn sword_to_garm_request_with_shots(sword: &Value, shots: u32) -> Result<Value> {
    let qubo = sword_to_qubo(sword)?;
    Ok(serde_json::json!({
        "matrix": qubo.matrix,
        "shots": shots
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_sword(path: &str) -> Value {
        json!({
            "recommended_path": path,
            "partition": {
                "graph": {
                    "nodes": [
                        {"id": 0, "h_i": 0.5},
                        {"id": 1, "h_i": -0.3},
                        {"id": 2, "h_i": 0.1}
                    ],
                    "edges": [
                        {"i": 0, "j": 1, "j_ij": 0.7},
                        {"i": 1, "j": 2, "j_ij": 0.2}
                    ]
                },
                "volatile_islands": [
                    {"node_ids": [0, 1]}
                ],
                "stable_edges": [
                    {"i": 1, "j": 2, "j_ij": 0.2}
                ]
            },
            "provenance": {
                "extractor": "distance",
                "domain": "optimization"
            },
            "noise": {"structural": 0.1, "environmental": 0.05}
        })
    }

    // ── sword_to_qubo ────────────────────────────────────────────────────

    #[test]
    fn qubo_size_matches_volatile_count() {
        let q = sword_to_qubo(&minimal_sword("classical_optimization")).unwrap();
        assert_eq!(q.n, 2);
        assert_eq!(q.matrix.len(), 2);
        assert_eq!(q.matrix[0].len(), 2);
        assert_eq!(q.index_to_node, vec![0, 1]);
    }

    #[test]
    fn qubo_diagonal_is_2h() {
        let q = sword_to_qubo(&minimal_sword("classical_optimization")).unwrap();
        // node 0 → h_i=0.5 → Q[0][0]=1.0
        assert!((q.matrix[0][0] - 1.0).abs() < 1e-12, "Q[0][0] should be 2*0.5=1.0");
        // node 1 → h_i=-0.3 → Q[1][1]=-0.6
        assert!((q.matrix[1][1] - (-0.6)).abs() < 1e-12, "Q[1][1] should be 2*(-0.3)=-0.6");
    }

    #[test]
    fn qubo_off_diagonal_is_4j() {
        let q = sword_to_qubo(&minimal_sword("classical_optimization")).unwrap();
        // edge (0,1) j_ij=0.7 → Q[0][1]=4*0.7=2.8
        assert!((q.matrix[0][1] - 2.8).abs() < 1e-12, "Q[0][1] should be 4*0.7=2.8");
        // lower triangle stays 0
        assert!((q.matrix[1][0]).abs() < 1e-12, "Q[1][0] should be 0 (upper-triangular)");
    }

    #[test]
    fn qubo_excludes_stable_only_edges() {
        // Edge (1,2) has node 2 stable — should not appear in 2×2 QUBO.
        let q = sword_to_qubo(&minimal_sword("classical_optimization")).unwrap();
        assert_eq!(q.n, 2); // node 2 not in matrix
    }

    // ── sword_to_ising ───────────────────────────────────────────────────

    #[test]
    fn ising_h_matches_node_fields() {
        let ising = sword_to_ising(&minimal_sword("classical_optimization")).unwrap();
        // node 0 local idx 0: h_i=0.5, plus boundary from edge (1,2)? No — edge
        // (1,2) has j_ij=0.2 and node 2 is stable. Node 1 is volatile, so it
        // gains the mean-field shift: h[1] += 0.2.
        assert!((ising.h[0] - 0.5).abs() < 1e-12, "h[0] should be 0.5");
        assert!((ising.h[1] - (-0.1)).abs() < 1e-12, "h[1] = -0.3 + 0.2 = -0.1");
    }

    #[test]
    fn ising_j_map_contains_volatile_coupling() {
        let ising = sword_to_ising(&minimal_sword("classical_optimization")).unwrap();
        // edge (0,1) j_ij=0.7
        assert!(ising.j.contains_key(&(0, 1)));
        assert!((ising.j[&(0, 1)] - 0.7).abs() < 1e-12);
    }

    #[test]
    fn ising_index_map_is_sorted() {
        let ising = sword_to_ising(&minimal_sword("classical_optimization")).unwrap();
        assert_eq!(ising.index_to_node, vec![0, 1]);
    }

    // ── sword_to_routing ─────────────────────────────────────────────────

    #[test]
    fn routing_volatile_segments_only_volatile_edges() {
        let r = sword_to_routing(&minimal_sword("classical_optimization")).unwrap();
        // Only edge (0,1) is between two volatile nodes.
        assert_eq!(r.volatile_segments.len(), 1);
        assert_eq!(r.volatile_segments[0], [0, 1]);
    }

    #[test]
    fn routing_fixed_segments_from_stable_edges() {
        let r = sword_to_routing(&minimal_sword("classical_optimization")).unwrap();
        assert_eq!(r.fixed_segments.len(), 1);
        assert_eq!(r.fixed_segments[0], [1, 2]);
    }

    #[test]
    fn routing_adjacency_is_bidirectional() {
        let r = sword_to_routing(&minimal_sword("classical_optimization")).unwrap();
        assert!(r.adjacency[&0].contains(&1));
        assert!(r.adjacency[&1].contains(&0));
    }

    // ── sword_to_garm_request ────────────────────────────────────────────

    #[test]
    fn garm_request_has_matrix_and_shots() {
        let req = sword_to_garm_request(&minimal_sword("classical_optimization")).unwrap();
        assert!(req.get("matrix").is_some());
        assert_eq!(req["shots"].as_u64().unwrap(), DEFAULT_SHOTS as u64);
    }

    #[test]
    fn garm_request_matrix_is_square_nxn() {
        let req = sword_to_garm_request(&minimal_sword("classical_optimization")).unwrap();
        let matrix = req["matrix"].as_array().unwrap();
        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].as_array().unwrap().len(), 2);
    }

    #[test]
    fn garm_request_with_custom_shots() {
        let req = sword_to_garm_request_with_shots(
            &minimal_sword("classical_optimization"),
            512,
        )
        .unwrap();
        assert_eq!(req["shots"].as_u64().unwrap(), 512);
    }

    // ── Error cases ──────────────────────────────────────────────────────

    #[test]
    fn error_on_missing_partition() {
        let sword = json!({"recommended_path": "classical_optimization"});
        assert!(matches!(
            sword_to_qubo(&sword),
            Err(AdapterError::MissingField("partition"))
        ));
    }

    #[test]
    fn error_on_empty_volatile_islands() {
        let sword = json!({
            "recommended_path": "classical_optimization",
            "partition": {
                "graph": {
                    "nodes": [{"id": 0, "h_i": 1.0}],
                    "edges": []
                },
                "volatile_islands": [],
                "stable_edges": []
            }
        });
        assert!(matches!(
            sword_to_qubo(&sword),
            Err(AdapterError::NoVolatileIslands)
        ));
    }

    #[test]
    fn accepts_any_recommended_path() {
        // Classical adapter should not gate on recommended_path.
        for path in &["classical_optimization", "black_box", "quantum"] {
            sword_to_qubo(&minimal_sword(path)).unwrap();
        }
    }
}
