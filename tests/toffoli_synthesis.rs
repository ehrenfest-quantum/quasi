// SPDX-License-Identifier: AGPL-3.0-or-later

use afana::ast::{Gate, GateName, EhrenfestAst, Measure};
use afana::emit::{emit_qasm, QasmVersion};
use afana::synthesis::synthesize_entangling_gates;

#[test]
fn toffoli_emits_ccx() {
    // Build an AST containing an explicit Toffoli (ccx) gate.
    let ast = EhrenfestAst {
        name: "toffoli".into(),
        n_qubits: 3,
        prepare: None,
        gates: vec![Gate {
            name: GateName::Ccx,
            qubits: vec![0, 1, 2],
            params: vec![],
        }],
        measures: vec![
            Measure { qubit: 0, cbit: 0 },
            Measure { qubit: 1, cbit: 1 },
            Measure { qubit: 2, cbit: 2 },
        ],
        conditionals: Vec::new(),
        expects: Vec::new(),
        type_decls: Vec::new(),
        variational_loops: Vec::new(),
    };

    // Verify synthesis recognises the Toffoli gate.
    let synth_result = synthesize_entangling_gates(&ast.gates);
    assert_eq!(synth_result.entangling_gates.len(), 1);
    assert_eq!(synth_result.entangling_gates[0].name, GateName::Ccx);

    // Emit QASM3 and ensure the ccx instruction appears.
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM emission failed");
    assert!(qasm.contains("ccx"), "QASM output should contain ccx instruction: {}", qasm);
}
