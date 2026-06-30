// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR intermediate representation for the Afana compiler.
//!
//! A proper ZX calculus graph representation where quantum gates are
//! decomposed into Z- and X-spiders connected by edges. This IR sits
//! between the gate-level [`EhrenfestAst`] and future ZX-based
//! optimization / extraction passes.
//!
//! Each qubit is a "wire" through the graph: an input boundary node,
//! a chain of spiders (one per gate on that qubit), and an output
//! boundary node.

use thiserror::Error;

/// A node identifier in the ZX graph.
pub type NodeId = usize;

/// Spider color in the ZX calculus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiderColor {
    Z,
    X,
}

impl std::fmt::Display for SpiderColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Z => write!(f, "Z"),
            Self::X => write!(f, "X"),
        }
    }
}

/// A spider node in the ZX graph.
#[derive(Debug, Clone)]
pub struct Spider {
    /// Z or X spider.
    pub color: SpiderColor,
    /// Phase in multiples of pi (e.g. 0.5 means pi/2).
    pub phase: f64,
    /// Which qubit this spider originated from, for tracking.
    pub qubit: Option<usize>,
}

/// Validation errors for ZX graphs.
#[derive(Debug, Error)]
pub enum ZxValidationError {
    #[error("invalid edge ({from}, {to}): {reason}")]
    InvalidEdge {
        from: NodeId,
        to: NodeId,
        reason: String,
    },

    #[error("invalid phase at node {node}: {phase} is not finite")]
    InvalidPhase { node: NodeId, phase: f64 },

    #[error("phase out of range at node {node}: {phase} not in [0, 2) (multiples of π)")]
    PhaseOutOfRange { node: NodeId, phase: f64 },

    #[error("nodes {a} and {b} are an unfused same-color pair on qubit {qubit}")]
    UnfusedAdjacentSpiders {
        a: NodeId,
        b: NodeId,
        qubit: usize,
    },

    #[error("invalid boundary node {node}: {reason}")]
    InvalidBoundary { node: NodeId, reason: String },

    #[error("structural error: {0}")]
    StructuralError(String),

    #[error("graph is disconnected: {unvisited} nodes unreachable")]
    DisconnectedGraph { unvisited: usize },
}

/// A ZX calculus graph.
///
/// Nodes are spiders (Z or X, each with a phase). Edges are undirected
/// and connect spiders. Boundary nodes mark the input/output wires of
/// the circuit.
#[derive(Debug, Clone)]
pub struct ZxGraph {
    spiders: Vec<Spider>,
    edges: Vec<(NodeId, NodeId)>,
    inputs: Vec<NodeId>,
    outputs: Vec<NodeId>,
}

impl ZxGraph {
    /// Create an empty ZX graph.
    pub fn new() -> Self {
        Self {
            spiders: Vec::new(),
            edges: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Add a spider to the graph and return its node ID.
    pub fn add_spider(&mut self, color: SpiderColor, phase: f64, qubit: Option<usize>) -> NodeId {
        let id = self.spiders.len();
        self.spiders.push(Spider {
            color,
            phase,
            qubit,
        });
        id
    }

    /// Add an undirected edge between two nodes.
    pub fn add_edge(&mut self, a: NodeId, b: NodeId) {
        self.edges.push((a, b));
    }

    /// Set the input boundary nodes.
    pub fn set_inputs(&mut self, nodes: Vec<NodeId>) {
        self.inputs = nodes;
    }

    /// Set the output boundary nodes.
    pub fn set_outputs(&mut self, nodes: Vec<NodeId>) {
        self.outputs = nodes;
    }

    /// Get a reference to a spider by ID.
    pub fn spider(&self, id: NodeId) -> &Spider {
        &self.spiders[id]
    }

    /// Get an iterator over all spiders in insertion order.
    pub fn spiders(&self) -> impl Iterator<Item = &Spider> + '_ {
        self.spiders.iter()
    }

    /// Get all neighbors of a node (via undirected edges).
    pub fn neighbors(&self, id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        for &(a, b) in &self.edges {
            if a == id {
                result.push(b);
            } else if b == id {
                result.push(a);
            }
        }
        result
    }

    /// Number of spiders in the graph.
    pub fn spider_count(&self) -> usize {
        self.spiders.len()
    }

    /// Check if the graph has no spiders.
    ///
    /// # Returns
    ///
    /// `true` if the graph contains zero spiders.
    pub fn is_empty(&self) -> bool {
        self.spider_count() == 0
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns the number of spiders whose color is `SpiderColor::Z`.
    pub fn count_z_spiders(&self) -> usize {
        self.spiders.iter().filter(|s| s.color == SpiderColor::Z).count()
    }

    /// Returns the number of spiders whose color is `SpiderColor::X`.
    pub fn count_x_spiders(&self) -> usize {
        self.spiders.iter().filter(|s| s.color == SpiderColor::X).count()
    }

    /// Validate the structural integrity of the graph.
    ///
    /// Checks:
    /// - All edge endpoints reference valid nodes
    /// - Input/output boundary nodes reference valid nodes
    /// - No self-loops
    /// - All phases are finite
    pub fn validate(&self) -> Result<(), Vec<ZxValidationError>> {
        let mut errors = Vec::new();
        let n = self.spiders.len();

        // Check edges.
        for &(a, b) in &self.edges {
            if a >= n || b >= n {
                errors.push(ZxValidationError::InvalidEdge {
                    from: a,
                    to: b,
                    reason: format!(
                        "endpoint out of range (graph has {} nodes)",
                        n
                    ),
                });
            }
            if a == b {
                errors.push(ZxValidationError::InvalidEdge {
                    from: a,
                    to: b,
                    reason: "self-loop".to_string(),
                });
            }
        }

        // Check phases.
        for (i, spider) in self.spiders.iter().enumerate() {
            if !spider.phase.is_finite() {
                errors.push(ZxValidationError::InvalidPhase {
                    node: i,
                    phase: spider.phase,
                });
            }
        }

        // Check boundary nodes.
        for &node in &self.inputs {
            if node >= n {
                errors.push(ZxValidationError::InvalidBoundary {
                    node,
                    reason: format!(
                        "input node out of range (graph has {} nodes)",
                        n
                    ),
                });
            }
        }
        for &node in &self.outputs {
            if node >= n {
                errors.push(ZxValidationError::InvalidBoundary {
                    node,
                    reason: format!(
                        "output node out of range (graph has {} nodes)",
                        n
                    ),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Stricter variant of [`Self::validate`] that additionally rejects
    /// spider phases outside the canonical range `[0, 2)` (in units of π).
    ///
    /// Used as the final gate before QASM emission: after [`Self::normalize_phases`]
    /// has run, any phase still outside `[0, 2)` indicates an upstream bug.
    /// Lowering passes routinely produce un-normalised phases (e.g. `-0.5`
    /// for `Sdg`, or `theta / π` for `Rx(θ)`), so [`Self::validate`] must
    /// stay permissive — call this method only after normalization.
    pub fn validate_normalized(&self) -> Result<(), Vec<ZxValidationError>> {
        let mut errors = match self.validate() {
            Ok(()) => Vec::new(),
            Err(e) => e,
        };

        for (i, spider) in self.spiders.iter().enumerate() {
            if !spider.phase.is_finite() {
                // Already reported by validate(); skip to avoid duplicate noise.
                continue;
            }
            if !(0.0..2.0).contains(&spider.phase) {
                errors.push(ZxValidationError::PhaseOutOfRange {
                    node: i,
                    phase: spider.phase,
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Reduce every spider phase to its canonical representative in
    /// `[0, 2)` (in units of π). ZX-calculus phases are equivalent modulo
    /// 2π, so normalisation is semantics-preserving.
    ///
    /// No-op for spiders whose phase is non-finite — those are caught by
    /// [`Self::validate`].
    pub fn normalize_phases(&mut self) {
        for spider in self.spiders.iter_mut() {
            if !spider.phase.is_finite() {
                continue;
            }
            // Rust's `%` keeps the sign of the dividend: ((-0.5) % 2.0) == -0.5.
            // The (x % 2 + 2) % 2 idiom guarantees a result in [0, 2).
            let p = spider.phase.rem_euclid(2.0);
            spider.phase = p;
        }
    }

    /// Find all pairs of spiders that should fuse under the ZX-calculus
    /// spider-fusion rule: two spiders connected by an edge, sharing the
    /// same color, both tagged with the same qubit-wire. Such a pair
    /// composes to a single spider with summed phase — leaving them
    /// un-fused on a canonical graph is a bug in the optimiser.
    ///
    /// The check captures the "parity" property of single-qubit gate
    /// sequences: an un-fused pair of phase-1 X-spiders represents X²
    /// (identity), an un-fused pair of phase-0 Z-spiders is also the
    /// identity, etc. Validation surfaces these residual sequences so a
    /// downstream pass can collapse them.
    ///
    /// Returns `(a, b)` pairs with `a < b`. Each pair appears at most once
    /// regardless of how many edges connect the two nodes.
    pub fn find_fusible_pairs(&self) -> Vec<(NodeId, NodeId)> {
        let mut seen: std::collections::HashSet<(NodeId, NodeId)> =
            std::collections::HashSet::new();

        for &(raw_a, raw_b) in &self.edges {
            if raw_a == raw_b {
                continue; // self-loop, already flagged by validate()
            }
            let (a, b) = if raw_a < raw_b {
                (raw_a, raw_b)
            } else {
                (raw_b, raw_a)
            };
            if a >= self.spiders.len() || b >= self.spiders.len() {
                continue; // out-of-range, already flagged by validate()
            }

            let sa = &self.spiders[a];
            let sb = &self.spiders[b];

            if sa.color != sb.color {
                continue;
            }
            match (sa.qubit, sb.qubit) {
                (Some(qa), Some(qb)) if qa == qb => {
                    seen.insert((a, b));
                }
                _ => {}
            }
        }

        let mut out: Vec<(NodeId, NodeId)> = seen.into_iter().collect();
        out.sort();
        out
    }

    /// Strict validator for the post-optimisation canonical form: every
    /// check in [`Self::validate_normalized`] plus a guarantee that no
    /// fusible spider pairs remain (see [`Self::find_fusible_pairs`]).
    ///
    /// Intended call site: immediately before QASM emission, after all
    /// optimisation passes have run.
    pub fn validate_fully_fused(&self) -> Result<(), Vec<ZxValidationError>> {
        let mut errors = match self.validate_normalized() {
            Ok(()) => Vec::new(),
            Err(e) => e,
        };

        for (a, b) in self.find_fusible_pairs() {
            // qubit is guaranteed present and equal by find_fusible_pairs.
            let qubit = self.spiders[a].qubit.expect("checked in find_fusible_pairs");
            errors.push(ZxValidationError::UnfusedAdjacentSpiders { a, b, qubit });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for ZxGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_is_valid() {
        let g = ZxGraph::new();
        assert!(g.validate().is_ok());
        assert_eq!(g.spider_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn add_spiders_and_edges() {
        let mut g = ZxGraph::new();
        let a = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let b = g.add_spider(SpiderColor::X, 0.0, Some(0));
        g.add_edge(a, b);

        assert_eq!(g.spider_count(), 2);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.spider(a).color, SpiderColor::Z);
        assert_eq!(g.spider(b).color, SpiderColor::X);
        assert_eq!(g.neighbors(a), vec![b]);
        assert_eq!(g.neighbors(b), vec![a]);
        assert!(g.validate().is_ok());
    }

    #[test]
    fn boundary_nodes() {
        let mut g = ZxGraph::new();
        let inp = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let mid = g.add_spider(SpiderColor::X, 0.5, Some(0));
        let out = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        g.add_edge(inp, mid);
        g.add_edge(mid, out);
        g.set_inputs(vec![inp]);
        g.set_outputs(vec![out]);

        assert!(g.validate().is_ok());
    }

    #[test]
    fn self_loop_detected() {
        let mut g = ZxGraph::new();
        let a = g.add_spider(SpiderColor::Z, 0.0, None);
        g.add_edge(a, a);

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidEdge { .. })));
    }

    #[test]
    fn invalid_edge_endpoint_detected() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 0.0, None);
        g.add_edge(0, 99); // 99 does not exist

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidEdge { .. })));
    }

    #[test]
    fn non_finite_phase_detected() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, f64::NAN, None);

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidPhase { .. })));
    }

    #[test]
    fn infinite_phase_detected() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::X, f64::INFINITY, None);

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidPhase { .. })));
    }

    #[test]
    fn invalid_boundary_detected() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 0.0, None);
        g.set_inputs(vec![42]); // does not exist

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidBoundary { .. })));
    }

    #[test]
    fn invalid_output_boundary_detected() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 0.0, None);
        g.set_outputs(vec![99]);

        let errs = g.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ZxValidationError::InvalidBoundary { .. })));
    }

    #[test]
    fn neighbors_with_multiple_edges() {
        let mut g = ZxGraph::new();
        let a = g.add_spider(SpiderColor::Z, 0.0, None);
        let b = g.add_spider(SpiderColor::X, 0.0, None);
        let c = g.add_spider(SpiderColor::Z, 0.5, None);
        g.add_edge(a, b);
        g.add_edge(a, c);

        let mut n = g.neighbors(a);
        n.sort();
        assert_eq!(n, vec![b, c]);
    }

    // ── Phase normalization + strict validation ────────────────────────────

    #[test]
    fn validate_normalized_accepts_in_range() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 0.0, None);
        g.add_spider(SpiderColor::X, 0.5, None);
        g.add_spider(SpiderColor::Z, 1.9999, None);
        assert!(g.validate_normalized().is_ok());
    }

    #[test]
    fn validate_normalized_rejects_negative_phase() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, -0.5, None);
        let errs = g.validate_normalized().unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e,
            ZxValidationError::PhaseOutOfRange { phase, .. } if *phase == -0.5
        )));
    }

    #[test]
    fn validate_normalized_rejects_phase_at_or_above_two() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 2.0, None);
        g.add_spider(SpiderColor::X, 7.0, None);
        let errs = g.validate_normalized().unwrap_err();
        let out_of_range: Vec<_> = errs
            .iter()
            .filter(|e| matches!(e, ZxValidationError::PhaseOutOfRange { .. }))
            .collect();
        assert_eq!(out_of_range.len(), 2);
    }

    #[test]
    fn normalize_phases_brings_negative_into_range() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, -0.5, None);
        g.normalize_phases();
        assert!((g.spider(0).phase - 1.5).abs() < 1e-12);
    }

    #[test]
    fn normalize_phases_brings_large_into_range() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, 7.0, None);
        g.normalize_phases();
        // 7.0 mod 2 = 1.0
        assert!((g.spider(0).phase - 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_then_validate_succeeds() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, -0.5, None);
        g.add_spider(SpiderColor::X, 7.0, None);
        g.add_spider(SpiderColor::Z, 100.25, None);
        g.normalize_phases();
        assert!(g.validate_normalized().is_ok());
    }

    #[test]
    fn normalize_phases_leaves_nan_alone() {
        let mut g = ZxGraph::new();
        g.add_spider(SpiderColor::Z, f64::NAN, None);
        g.normalize_phases();
        assert!(g.spider(0).phase.is_nan());
    }

    // ── Fusible-pair detection (single-qubit "parity" invariant) ───────────

    #[test]
    fn find_fusible_pairs_on_canonical_h_chain_is_empty() {
        // H lowers to Z-X-Z chain on a single qubit. No same-color
        // adjacency, so nothing to fuse.
        let mut g = ZxGraph::new();
        let z1 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let x = g.add_spider(SpiderColor::X, 0.0, Some(0));
        let z2 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        g.add_edge(z1, x);
        g.add_edge(x, z2);
        assert!(g.find_fusible_pairs().is_empty());
    }

    #[test]
    fn find_fusible_pairs_detects_z_z_chain() {
        // Two Z gates back-to-back lower to two Z-spiders on the same
        // qubit, edge-connected: a textbook fusible pair.
        let mut g = ZxGraph::new();
        let z1 = g.add_spider(SpiderColor::Z, 1.0, Some(0));
        let z2 = g.add_spider(SpiderColor::Z, 1.0, Some(0));
        g.add_edge(z1, z2);
        assert_eq!(g.find_fusible_pairs(), vec![(z1, z2)]);
    }

    #[test]
    fn find_fusible_pairs_ignores_cross_qubit_same_color() {
        // Z-spider on qubit 0 and Z-spider on qubit 1, connected by an
        // edge (e.g. a CZ entangler): same color but DIFFERENT qubits,
        // not fusible.
        let mut g = ZxGraph::new();
        let z0 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let z1 = g.add_spider(SpiderColor::Z, 0.0, Some(1));
        g.add_edge(z0, z1);
        assert!(g.find_fusible_pairs().is_empty());
    }

    #[test]
    fn find_fusible_pairs_ignores_untagged_spiders() {
        // Boundary nodes (qubit = None) are never fusible — they're
        // structural anchors.
        let mut g = ZxGraph::new();
        let a = g.add_spider(SpiderColor::Z, 0.0, None);
        let b = g.add_spider(SpiderColor::Z, 0.0, None);
        g.add_edge(a, b);
        assert!(g.find_fusible_pairs().is_empty());
    }

    #[test]
    fn validate_fully_fused_rejects_unfused_x_x_pair() {
        // Two X-spiders on the same qubit: X² = I. Parity-2 single-qubit
        // sequence that should have collapsed.
        let mut g = ZxGraph::new();
        let x1 = g.add_spider(SpiderColor::X, 1.0, Some(0));
        let x2 = g.add_spider(SpiderColor::X, 1.0, Some(0));
        g.add_edge(x1, x2);
        let errs = g.validate_fully_fused().unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e,
            ZxValidationError::UnfusedAdjacentSpiders { qubit: 0, .. }
        )));
    }

    #[test]
    fn validate_fully_fused_accepts_canonical_h() {
        let mut g = ZxGraph::new();
        let z1 = g.add_spider(SpiderColor::Z, 0.5, Some(0));
        let x = g.add_spider(SpiderColor::X, 0.5, Some(0));
        let z2 = g.add_spider(SpiderColor::Z, 0.5, Some(0));
        g.add_edge(z1, x);
        g.add_edge(x, z2);
        assert!(g.validate_fully_fused().is_ok());
    }

    #[test]
    fn count_z_and_x_spiders() {
        let mut g = ZxGraph::new();
        let _z1 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let _z2 = g.add_spider(SpiderColor::Z, 0.0, Some(0));
        let _x1 = g.add_spider(SpiderColor::X, 0.0, Some(0));

        assert_eq!(g.count_z_spiders(), 2);
        assert_eq!(g.count_x_spiders(), 1);
    }
}
