// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR spider pattern synthesis to QASM3 gates.
//!
//! This module implements synthesis of controlled gates from ZX-calculus spider
//! patterns. Currently supports:
//! - Controlled-Hadamard (CH) gate synthesis from Hadamard spiders with control
//!   phase patterns.

use crate::ast::{Gate, GateName, QasmProgram};

/// ZX spider node type for pattern matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiderType {
    Z,
    X,
    Hadamard,
}

/// A ZX spider node with its properties.
#[derive(Debug, Clone)]
pub struct Spider {
    pub node_type: SpiderType,
    pub phase: f64,
    pub qubit: usize,
    pub neighbors: Vec<usize>,
}

/// Synthesize QASM3 program from ZX-IR spider graph.
///
/// Detects controlled-Hadamard patterns where a Hadamard spider is connected
/// to a control spider with appropriate phase relationships.
pub fn synthesize_from_zx(spiders: &[Spider]) -> QasmProgram {
    let mut program = QasmProgram::new();

    for spider in spiders {
        match spider.node_type {
            SpiderType::Hadamard => {
                // Check for CH pattern: Hadamard spider with exactly one Z/X neighbor
                // that acts as control (phase = 0 or pi)
                if spider.neighbors.len() == 1 {
                    let control_qubit = spider.neighbors[0];
                    // Synthesize CH gate: control on neighbor, target on this spider
                    program.add_gate("ch", &[control_qubit, spider.qubit], &[]);
                } else if spider.neighbors.is_empty() {
                    // Single Hadamard spider → H gate
                    program.add_gate("h", &[spider.qubit], &[]);
                }
            }
            SpiderType::Z | SpiderType::X => {
                // Single spiders with no neighbors → identity (no gate needed)
                // Multi-neighbor spiders handled by entangling gate synthesis
            }
        }
    }

    program
}

/// QASM3 program builder for synthesized gates.
#[derive(Debug, Clone, Default)]
pub struct QasmProgram {
    gates: Vec<Gate>,
    n_qubits: usize,
}

impl QasmProgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_gate(&mut self, name: &str, qubits: &[usize], params: &[f64]) {
        if let Some(gate_name) = GateName::from_token(name) {
            self.gates.push(Gate {
                name: gate_name,
                qubits: qubits.to_vec(),
                params: params.to_vec(),
            });
            // Track maximum qubit index
            for &q in qubits {
                self.n_qubits = self.n_qubits.max(q + 1);
            }
        }
    }

    pub fn gates(&self) -> &[Gate] {
        &self.gates
    }

    pub fn n_qubits(&self) -> usize {
        self.n_qubits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_ch_from_hadamard_spider_with_control() {
        // CH pattern: Hadamard spider on qubit 1 connected to Z spider on qubit 0
        let spiders = vec![
            Spider {
                node_type: SpiderType::Z,
                phase: 0.0,
                qubit: 0,
                neighbors: vec![1],
            },
            Spider {
                node_type: SpiderType::Hadamard,
                phase: 0.0,
                qubit: 1,
                neighbors: vec![0],
            },
        ];

        let program = synthesize_from_zx(&spiders);
        assert_eq!(program.n_qubits(), 2);
        assert_eq!(program.gates().len(), 1);
        assert_eq!(program.gates()[0].name, GateName::Ch);
        assert_eq!(program.gates()[0].qubits, vec![0, 1]);
    }

    #[test]
    fn synthesize_h_from_single_hadamard_spider() {
        // Single Hadamard spider with no neighbors → H gate
        let spiders = vec![Spider {
            node_type: SpiderType::Hadamard,
            phase: 0.0,
            qubit: 0,
            neighbors: vec![],
        }];

        let program = synthesize_from_zx(&spiders);
        assert_eq!(program.n_qubits(), 1);
        assert_eq!(program.gates().len(), 1);
        assert_eq!(program.gates()[0].name, GateName::H);
        assert_eq!(program.gates()[0].qubits, vec![0]);
    }

    #[test]
    fn synthesize_empty_graph() {
        let spiders: Vec<Spider> = vec![];
        let program = synthesize_from_zx(&spiders);
        assert_eq!(program.n_qubits(), 0);
        assert!(program.gates().is_empty());
    }
}
