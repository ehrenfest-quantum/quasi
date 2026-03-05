use std::process::Command;

#[test]
fn test_type_checker_rejects_invalid_program() {
    // Create a temporary file with a type error (assigning classical int to qubit)
    let invalid_program = r#"
program \"invalid\"
qubits 2
// This should fail: h gate expects a qubit, not an integer
h 42
    "#;
    
    std::fs::write("test_invalid.ef", invalid_program).unwrap();
    
    // Run the compiler on the invalid program
    let output = Command::new("cargo")
        .args(["run", "--", "test_invalid.ef"])
        .current_dir("afana")
        .output()
        .expect("Failed to execute compiler");
    
    // Clean up
    std::fs::remove_file("test_invalid.ef").unwrap();
    
    // The compiler should return a non-zero exit code for type errors
    assert!(!output.status.success(), "Compiler should fail on type error");
    
    // The error message should mention type checking
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("type") || stderr.contains("Type") || stderr.contains("arity"),
        "Error message should indicate a type error, got: {}",
        stderr
    );
}

#[test]
fn test_type_checker_accepts_valid_program() {
    let valid_program = r#"
program \"valid\"
qubits 2
h q0
cnot q0 q1
measure q0 -> c0
    "#;
    
    std::fs::write("test_valid.ef", valid_program).unwrap();
    
    // Run the compiler on the valid program
    let output = Command::new("cargo")
        .args(["run", "--", "test_valid.ef"])
        .current_dir("afana")
        .output()
        .expect("Failed to execute compiler");
    
    // Clean up
    std::fs::remove_file("test_valid.ef").unwrap();
    
    // The compiler should succeed on valid programs
    assert!(output.status.success(), "Compiler should succeed on valid program");
}

#[test]
fn test_qubit_out_of_range_rejected() {
    let program = r#"
program \"out_of_range\"
qubits 2
h q5
    "#;
    
    std::fs::write("test_out_of_range.ef", program).unwrap();
    
    let output = Command::new("cargo")
        .args(["run", "--", "test_out_of_range.ef"])
        .current_dir("afana")
        .output()
        .expect("Failed to execute compiler");
    
    std::fs::remove_file("test_out_of_range.ef").unwrap();
    
    assert!(!output.status.success(), "Compiler should reject out-of-range qubits");
}

#[test]
fn test_gate_arity_mismatch_rejected() {
    let program = r#"
program \"arity\"
qubits 2
// h gate takes 1 qubit, not 2
h q0 q1
    "#;
    
    std::fs::write("test_arity.ef", program).unwrap();
    
    let output = Command::new("cargo")
        .args(["run", "--", "test_arity.ef"])
        .current_dir("afana")
        .output()
        .expect("Failed to execute compiler");
    
    std::fs::remove_file("test_arity.ef").unwrap();
    
    assert!(!output.status.success(), "Compiler should reject gate arity mismatch");
}