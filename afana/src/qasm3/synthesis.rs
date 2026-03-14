// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! ZX-IR to QASM3 synthesis module.
//!
//! Converts ZX-calculus spiders (ZxNode) into QASM3 operations
//! for hardware-compatible gate emission.

/// Gate types for ZX-calculus spiders.
#[derive(Debug, Clone, PartialEq)]
pub enum ZxGateType {
    /// Pauli-X gate spider
    X,
    /// Pauli-Y gate spider
    Y,
    /// Pauli-Z gate spider
    Z,
    /// Hadamard gate spider
    H,
}

/// A node in the ZX-IR graph representing a quantum operation.
#[derive(Debug, Clone, PartialEq)]
pub struct ZxNode {
    /// The type of gate this node represents
    pub gate_type: ZxGateType,
    /// The target qubit index
    pub qubit: usize,
}

/// A QASM3 operation ready for emission.
#[derive(Debug, Clone, PartialEq)]
pub enum Qasm3Operation {
    /// A quantum gate operation
    Gate {
        /// Gate name (e.g., "x", "y", "z", "h")
        name: String,
        /// Target qubit indices
        qubits: Vec<usize>,
        /// Gate parameters (empty for Clifford gates)
        params: Vec<f64>,
    },
}

impl From<ZxNode> for Qasm3Operation {
    fn from(node: ZxNode) -> Self {
        match node.gate_type {
            ZxGateType::X => {
                Qasm3Operation::Gate {
                    name: "x".to_string(),
                    qubits: vec![node.qubit],
                    params: vec![],
                }
            }
            ZxGateType::Y => {
                Qasm3Operation::Gate {
                    name: "y".to_string(),
                    qubits: vec![node.qubit],
                    params: vec![],
                }
            }
            ZxGateType::Z => {
                Qasm3Operation::Gate {
                    name: "z".to_string(),
                    qubits: vec![node.qubit],
                    params: vec![],
                }
            }
            ZxGateType::H => {
                Qasm3Operation::Gate {
                    name: "h".to_string(),
                    qubits: vec![node.qubit],
                    params: vec![],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_y_gate_synthesis() {
        let node = ZxNode {
            gate_type: ZxGateType::Y,
            qubit: 0,
        };
        let operation: Qasm3Operation = node.into();
        
        match operation {
            Qasm3Operation::Gate { name, qubits, params } => {
                assert_eq!(name, "y");
                assert_eq!(qubits, vec![0]);
                assert!(params.is_empty());
            }
        }
    }

    #[test]
    fn test_x_gate_synthesis() {
        let node = ZxNode {
            gate_type: ZxGateType::X,
            qubit: 1,
        };
        let operation: Qasm3Operation = node.into();
        
        match operation {
            Qasm3Operation::Gate { name, qubits, params } => {
                assert_eq!(name, "x");
                assert_eq!(qubits, vec![1]);
                assert!(params.is_empty());
            }
        }
    }

    #[test]
    fn test_z_gate_synthesis() {
        let node = ZxNode {
            gate_type: ZxGateType::Z,
            qubit: 2,
        };
        let operation: Qasm3Operation = node.into();
        
        match operation {
            Qasm3Operation::Gate { name, qubits, params } => {
                assert_eq!(name, "z");
                assert_eq!(qubits, vec![2]);
                assert!(params.is_empty());
            }
        }
    }

    #[test]
    fn test_h_gate_synthesis() {
        let node = ZxNode {
            gate_type: ZxGateType::H,
            qubit: 3,
        };
        let operation: Qasm3Operation = node.into();
        
        match operation {
            Qasm3Operation::Gate { name, qubits, params } => {
                assert_eq!(name, "h");
                assert_eq!(qubits, vec![3]);
                assert!(params.is_empty());
            }
        }
    }
}
