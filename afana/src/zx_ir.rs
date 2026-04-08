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

    #[error("invalid boundary node {node}: {reason}")]
    InvalidBoundary { node: NodeId, reason: String },

    #[error("structural error: {0}")]
    StructuralError(String),
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

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
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

        // Check wire label consistency: connected spiders must share the same
        // qubit label when both have one assigned.
        for &(a, b) in &self.edges {
            let qa = self.spiders[a].qubit;
            let qb = self.spiders[b].qubit;
            if let (Some(qa), Some(qb)) = (qa, qb) {
                if qa != qb {
                    errors.push(ZxValidationError::StructuralError(format!(
                        "wire label mismatch: edge ({a}, {b}) connects qubit {qa} to qubit {qb}"
                    )));
                }
            }
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
}
