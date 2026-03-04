//! Integration tests for QASM3 edge cases involving variational parameters and custom gate definitions.

use afana::parser;
use afana::emit::{self, QasmVersion};

#[test]
fn emit_variational_parameters_in_qasm3() {
    let source = r#"
program "vqe"
qubits 2
variational params theta phi max_iter 150
  rx theta q0
  ry phi q1
  cnot q0 q1
end
"#;
    let ast = parser::parse(source).expect("Parsing should succeed");
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("Emission should succeed");
    // Ensure input declarations for each variational parameter are present.
    assert!(qasm.contains("input float[64] theta;"), "Missing input for theta");
    assert!(qasm.contains("input float[64] phi;"), "Missing input for phi");
    // Ensure the variational comment line is present.
    assert!(qasm.contains("// Variational ansatz — max_iter=150"), "Missing variational comment");
}

#[test]
fn emit_multiple_variational_parameters_order() {
    let source = r#"
program "vqe2"
qubits 3
variational params a b c max_iter 10
  rx a q0
  ry b q1
  rz c q2
end
"#;
    let ast = parser::parse(source).expect("Parsing should succeed");
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("Emission should succeed");
    // Verify all three inputs appear in the correct order.
    let lines: Vec<&str> = qasm.lines().collect();
    let input_lines: Vec<&str> = lines.iter().filter(|l| l.trim_start().starts_with("input float[64]")).cloned().collect();
    assert_eq!(input_lines, ["input float[64] a;", "input float[64] b;", "input float[64] c;"]);
}

#[test]
fn parse_unknown_gate_fails() {
    let source = r#"
program "badgate"
qubits 1
mygate q0
"#;
    let err = parser::parse(source).expect_err("Parsing should fail for unknown gate");
    assert!(err.to_string().contains("unknown gate"), "Error message should mention unknown gate");
}

#[test]
fn parse_variational_without_params_fails() {
    let source = r#"
program "novars"
qubits 2
variational max_iter 5
  rx 0.5 q0
end
"#;
    let err = parser::parse(source).expect_err("Parsing should fail when variational params are missing");
    assert!(err.to_string().contains("'variational' syntax"), "Error should indicate variational syntax issue");
}

#[test]
fn parse_custom_gate_in_body_fails() {
    let source = r#"
program "customgate"
qubits 2
variational params theta max_iter 20
  mygate theta q0
end
"#;
    let err = parser::parse(source).expect_err("Parsing should fail for custom gate inside variational body");
    assert!(err.to_string().contains("unknown gate"), "Error should mention unknown gate in variational body");
}
