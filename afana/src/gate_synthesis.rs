// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Gate synthesis from ZX-IR phase angles.
//!
//! This module provides functions to synthesize quantum gates from ZX-calculus
//! phase angles, bridging the ZX optimization layer to concrete gate sequences.

use crate::ast::{Gate, GateName};
use crate::emit::{emit_qasm, QasmVersion};
use crate::ast::EhrenfestAst;

/// Synthesize a T† (T-dagger) gate from ZX-IR phase angles.
///
/// The T† gate is the inverse of the T gate, with phase angle -π/4.
/// In ZX-calculus, this corresponds to a Z-spider with phase -π/4.
/// This function returns a single `tdg` gate on the specified qubit.
///
/// # Arguments
/// * `qubit` - The target qubit index (0-based)
///
/// # Returns
/// A `Gate` representing the T† operation.
pub fn synth_t_dagger(qubit: usize) -> Gate {
    Gate {
        name: GateName::Tdg,
        qubits: vec![qubit],
        params: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_t_dagger() {
        // Test synthesis of T† gate
        let gate = synth_t_dagger(0);
        assert_eq!(gate.name, GateName::Tdg);
        assert_eq!(gate.qubits, vec![0]);
        assert!(gate.params.is_empty());

        // Test QASM3 emission
        let ast = EhrenfestAst {
            name: "test_t_dagger".into(),
            n_qubits: 1,
            prepare: None,
            gates: vec![gate],
            measures: vec![],
            conditionals: vec![],
            expects: vec![],
            type_decls: vec![],
            variational_loops: vec![],
        };
        
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("tdg q[0];"), "QASM3 should contain 'tdg' statement");
        
        // Verify it's valid QASM3
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("include \"stdgates.inc\";"));
    }
}
