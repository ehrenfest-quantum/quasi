// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR to QASM3 conversion with CZ gate synthesis.
//!
//! This module detects patterns in ZX diagrams (adjacent Z-spiders with phase π)
//! and synthesizes them into explicit CZ gates for QASM3 emission.

use crate::ast::{Gate, GateName};

/// A ZX spider node identifier.
pub type Node = usize;

/// Spider type in the ZX calculus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Z,
    X,
}

/// A minimal ZX graph representation for spider analysis.
#[derive(Debug, Clone)]
pub struct Graph {
    /// Spider types (Z or X).
    pub spider_types: Vec<Type>,
    /// Phase of each spider in radians.
    pub phases: Vec<f64>,
    /// Adjacency list representation of edges.
    pub edges: Vec<Vec<Node>>,
}

impl Graph {
    /// Get the type of a spider.
    pub fn get_type(&self, spider: Node) -> Type {
        self.spider_types[spider]
    }

    /// Get the phase of a spider.
    pub fn get_phase(&self, spider: Node) -> f64 {
        self.phases[spider]
    }

    /// Check if two spiders are connected by a single edge.
    pub fn connected_by_single_edge(&self, spider1: Node, spider2: Node) -> bool {
        if spider1 >= self.edges.len() || spider2 >= self.edges.len() {
            return false;
        }
        let neighbors = &self.edges[spider1];
        neighbors.iter().filter(|&&n| n == spider2).count() == 1
    }
}

/// A sequence of gates to emit.
pub type GateSequence = Vec<Gate>;

/// Decompose a pair of adjacent ZX spiders into a gate sequence.
///
/// Detects the pattern of two Z-spiders with phase π connected by a single edge,
/// which is equivalent to a CZ gate between their qubits.
///
/// Returns `Some(vec![cz_gate])` when the pattern matches, `None` otherwise.
pub fn decompose_spider_pair(
    graph: &Graph,
    spider1: Node,
    spider2: Node,
) -> Option<GateSequence> {
    let phase1 = graph.get_phase(spider1);
    let phase2 = graph.get_phase(spider2);
    
    // Check for adjacent Z-spiders with phase π
    if graph.get_type(spider1) == Type::Z
        && graph.get_type(spider2) == Type::Z
        && graph.connected_by_single_edge(spider1, spider2)
        && (phase1 - std::f64::consts::PI).abs() < 1e-10
        && (phase2 - std::f64::consts::PI).abs() < 1e-10
    {
        // Synthesize CZ gate between the two spider qubits
        Some(vec![Gate {
            name: GateName::Cz,
            qubits: vec![spider1, spider2],
            params: vec![],
        }])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::{emit_qasm, QasmVersion};

    /// Build a ZX graph representing two adjacent Z-spiders with phase π.
    fn cz_spider_graph() -> Graph {
        Graph {
            spider_types: vec![Type::Z, Type::Z],
            phases: vec![std::f64::consts::PI, std::f64::consts::PI],
            edges: vec![vec![1], vec![0]],
        }
    }

    #[test]
    fn adjacent_z_spiders_with_pi_phase_produce_cz() {
        let graph = cz_spider_graph();
        let result = decompose_spider_pair(&graph, 0, 1);
        
        assert!(result.is_some(), "Should detect CZ pattern");
        let gates = result.unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, GateName::Cz);
        assert_eq!(gates[0].qubits, vec![0, 1]);
    }

    #[test]
    fn non_pi_phase_does_not_produce_cz() {
        let mut graph = cz_spider_graph();
        graph.phases[0] = std::f64::consts::FRAC_PI_2; // π/2 instead of π
        
        let result = decompose_spider_pair(&graph, 0, 1);
        assert!(result.is_none(), "Should not produce CZ for non-π phase");
    }

    #[test]
    fn x_spider_does_not_produce_cz() {
        let mut graph = cz_spider_graph();
        graph.spider_types[0] = Type::X;
        
        let result = decompose_spider_pair(&graph, 0, 1);
        assert!(result.is_none(), "Should not produce CZ for X spider");
    }

    #[test]
    fn not_connected_does_not_produce_cz() {
        let mut graph = cz_spider_graph();
        graph.edges = vec![vec![], vec![]]; // No edges
        
        let result = decompose_spider_pair(&graph, 0, 1);
        assert!(result.is_none(), "Should not produce CZ when not connected");
    }

    #[test]
    fn cz_emits_valid_qasm3() {
        let graph = cz_spider_graph();
        let result = decompose_spider_pair(&graph, 0, 1).unwrap();
        
        // Create an AST with the synthesized CZ gate
        let ast = crate::ast::EhrenfestAst {
            name: "cz_test".into(),
            n_qubits: 2,
            prepare: None,
            gates: result,
            measures: vec![],
            conditionals: vec![],
            expects: vec![],
            type_decls: vec![],
            variational_loops: vec![],
        };
        
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("cz q[0], q[1];"), "QASM3 should contain cz statement");
        assert!(qasm.contains("OPENQASM 3.0;"), "Should be QASM3 format");
    }

    #[test]
    fn cz_emits_valid_qasm2() {
        let graph = cz_spider_graph();
        let result = decompose_spider_pair(&graph, 0, 1).unwrap();
        
        let ast = crate::ast::EhrenfestAst {
            name: "cz_test".into(),
            n_qubits: 2,
            prepare: None,
            gates: result,
            measures: vec![],
            conditionals: vec![],
            expects: vec![],
            type_decls: vec![],
            variational_loops: vec![],
        };
        
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("cz q[0], q[1];"), "QASM2 should contain cz statement");
    }
}