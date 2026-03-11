// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! QASM3 generation from ZX-IR nodes.

use crate::ast::{Gate, GateName, Measure};
use std::collections::HashMap;

/// ZX-IR node types for intermediate representation.
#[derive(Debug, Clone, PartialEq)]
pub enum ZXNode {
    /// Z-spider with phase (Z-basis rotation)
    ZSpider {
        id: usize,
        phase: f64,
    },
    /// X-spider with phase (X-basis rotation)
    XSpider {
        id: usize,
        phase: f64,
    },
    /// Hadamard gate
    Hadamard {
        id: usize,
    },
    /// Measurement spider - measures qubit and assigns to classical bit
    Measurement {
        id: usize,
        qubit_id: usize,
    },
    /// Two-qubit entangling gate
    Edge {
        source: usize,
        target: usize,
    },
}

impl ZXNode {
    pub fn id(&self) -> usize {
        match self {
            ZXNode::ZSpider { id, .. } => *id,
            ZXNode::XSpider { id, .. } => *id,
            ZXNode::Hadamard { id } => *id,
            ZXNode::Measurement { id, .. } => *id,
            ZXNode::Edge { source, .. } => *source,
        }
    }

    pub fn qubit_id(&self) -> Option<usize> {
        match self {
            ZXNode::ZSpider { .. } | ZXNode::XSpider { .. } | ZXNode::Hadamard { .. } |
            ZXNode::Measurement { .. } => Some(self.id()),
            ZXNode::Edge { source, .. } => Some(*source),
        }
    }
}

/// QASM3 generator for ZX-IR nodes.
pub struct Qasm3Generator {
    qubit_map: HashMap<usize, String>,
    classical_map: HashMap<usize, String>,
    next_clbit: usize,
}

impl Qasm3Generator {
    pub fn new() -> Self {
        Self {
            qubit_map: HashMap::new(),
            classical_map: HashMap::new(),
            next_clbit: 0,
        }
    }

    /// Declare a qubit and return its QASM3 name.
    pub fn declare_qubit(&mut self, zx_id: usize) -> String {
        let name = format!("q{}", zx_id);
        self.qubit_map.insert(zx_id, name.clone());
        name
    }

    /// Declare a classical bit and return its QASM3 name.
    pub fn declare_classical_bit(&mut self, zx_id: usize) -> String {
        let name = format!("c{}", self.next_clbit);
        self.next_clbit += 1;
        self.classical_map.insert(zx_id, name.clone());
        name
    }

    /// Convert a ZX-IR node to QASM3 statement.
    pub fn node_to_qasm3(&self, node: &ZXNode) -> Option<String> {
        match node {
            ZXNode::ZSpider { phase, .. } => {
                let qubit = self.qubit_map.get(&node.id())?;
                Some(format!("rz({}) {};", phase, qubit))
            }
            ZXNode::XSpider { phase, .. } => {
                let qubit = self.qubit_map.get(&node.id())?;
                Some(format!("rx({}) {};", phase, qubit))
            }
            ZXNode::Hadamard { .. } => {
                let qubit = self.qubit_map.get(&node.id())?;
                Some(format!("h {};", qubit))
            }
            ZXNode::Measurement { qubit_id, .. } => {
                // Get or declare the qubit
                let qubit = self.qubit_map.get(qubit_id)
                    .or_else(|| {
                        let name = format!("q{}", qubit_id);
                        self.qubit_map.insert(*qubit_id, name.clone());
                        Some(name)
                    })?;
                // Get or declare the classical bit
                let cbit = self.classical_map.get(&node.id())
                    .or_else(|| {
                        let name = self.declare_classical_bit(node.id());
                        Some(name)
                    })?;
                Some(format!("measure {} -> {};", qubit, cbit))
            }
            ZXNode::Edge { .. } => {
                // Edge represents entanglement, emit as CX
                Some("// Edge: entanglement".to_string())
            }
        }
    }

    /// Generate complete QASM3 program from ZX-IR nodes.
    pub fn generate_qasm3(&mut self, nodes: &[ZXNode], n_qubits: usize) -> String {
        let mut lines = Vec::new();
        lines.push("OPENQASM 3.0;".to_string());
        lines.push("include \"stdgates.inc\";".to_string());
        lines.push(String::new());
        lines.push(format!("qubit[{}] q;", n_qubits));
        
        // Count classical bits needed
        let mut max_cbit = 0;
        for node in nodes {
            if let ZXNode::Measurement { id, .. } = node {
                if let Some(name) = self.classical_map.get(id) {
                    if let Ok(idx) = name.strip_prefix("c").parse::<usize>() {
                        max_cbit = max_cbit.max(idx + 1);
                    }
                }
            }
        }
        if max_cbit > 0 {
            lines.push(format!("bit[{}] c;", max_cbit));
        }
        
        lines.push(String::new());
        
        // Emit each node
        for node in nodes {
            if let Some(stmt) = self.node_to_qasm3(node) {
                lines.push(stmt);
            }
        }
        
        lines.join("\n")
    }

    /// Extract measures from ZX-IR nodes.
    pub fn extract_measures(&self, nodes: &[ZXNode]) -> Vec<Measure> {
        let mut measures = Vec::new();
        for node in nodes {
            if let ZXNode::Measurement { id, qubit_id } = node {
                if let Some(cbit_name) = self.classical_map.get(id) {
                    if let Ok(cbit) = cbit_name.strip_prefix("c").parse::<usize>() {
                        measures.push(Measure {
                            qubit: *qubit_id,
                            cbit,
                        });
                    }
                }
            }
        }
        measures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qasm3_measurement_synthesis() {
        let mut gen = Qasm3Generator::new();
        
        // Declare qubits
        gen.declare_qubit(0);
        gen.declare_qubit(1);
        
        // Create measurement nodes
        let nodes = vec![
            ZXNode::Hadamard { id: 0 },
            ZXNode::Measurement { id: 0, qubit_id: 0 },
            ZXNode::Measurement { id: 1, qubit_id: 1 },
        ];
        
        let qasm = gen.generate_qasm3(&nodes, 2);
        
        // Verify QASM3 structure
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("qubit[2] q;"));
        assert!(qasm.contains("bit[2] c;"));
        
        // Verify measurement statements with classical register assignment
        assert!(qasm.contains("measure q[0] -> c[0];"));
        assert!(qasm.contains("measure q[1] -> c[1];"));
        
        // Verify Hadamard gate
        assert!(qasm.contains("h q[0];"));
    }

    #[test]
    fn qasm3_measurement_classical_allocation() {
        let mut gen = Qasm3Generator::new();
        
        // Create multiple measurements on same qubit
        let nodes = vec![
            ZXNode::Measurement { id: 0, qubit_id: 0 },
            ZXNode::Measurement { id: 1, qubit_id: 0 },
            ZXNode::Measurement { id: 2, qubit_id: 1 },
        ];
        
        let qasm = gen.generate_qasm3(&nodes, 2);
        
        // Each measurement should get unique classical bit
        assert!(qasm.contains("measure q[0] -> c[0];"));
        assert!(qasm.contains("measure q[0] -> c[1];"));
        assert!(qasm.contains("measure q[1] -> c[2];"));
    }

    #[test]
    fn qasm3_measurement_extract_measures() {
        let mut gen = Qasm3Generator::new();
        gen.declare_qubit(0);
        gen.declare_qubit(1);
        
        let nodes = vec![
            ZXNode::Measurement { id: 0, qubit_id: 0 },
            ZXNode::Measurement { id: 1, qubit_id: 1 },
        ];
        
        let measures = gen.extract_measures(&nodes);
        assert_eq!(measures.len(), 2);
        assert_eq!(measures[0].qubit, 0);
        assert_eq!(measures[0].cbit, 0);
        assert_eq!(measures[1].qubit, 1);
        assert_eq!(measures[1].cbit, 1);
    }

    #[test]
    fn qasm3_measurement_with_z_spider() {
        let mut gen = Qasm3Generator::new();
        gen.declare_qubit(0);
        
        let nodes = vec![
            ZXNode::ZSpider { id: 0, phase: std::f64::consts::FRAC_PI_2 },
            ZXNode::Measurement { id: 0, qubit_id: 0 },
        ];
        
        let qasm = gen.generate_qasm3(&nodes, 1);
        assert!(qasm.contains("rz(pi/2) q[0];"));
        assert!(qasm.contains("measure q[0] -> c[0];"));
    }

    #[test]
    fn qasm3_measurement_with_x_spider() {
        let mut gen = Qasm3Generator::new();
        gen.declare_qubit(0);
        
        let nodes = vec![
            ZXNode::XSpider { id: 0, phase: std::f64::consts::FRAC_PI_4 },
            ZXNode::Measurement { id: 0, qubit_id: 0 },
        ];
        
        let qasm = gen.generate_qasm3(&nodes, 1);
        assert!(qasm.contains("rx(pi/4) q[0];"));
        assert!(qasm.contains("measure q[0] -> c[0];"));
    }
}
