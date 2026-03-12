//! Integration tests for the Ehrenfest type system

use afana::{QuantumType, TypeChecker, Expr, Declaration, Statement};

/// Test parsing and type-checking a simple single-qubit program
#[test]
fn test_single_qubit_program() {
    let mut checker = TypeChecker::new();
    
    // Declare a single qubit register
    let decl = Declaration::QregDecl {
        name: "q".to_string(),
        size: 1,
    };
    assert!(checker.declare(&decl).is_ok());
    
    // Verify the type was registered
    assert_eq!(checker.env().get("q"), Some(&QuantumType::Qreg(1)));
    
    // Access the qubit
    let expr = Expr::QregIndex {
        qreg: "q".to_string(),
        index: 0,
    };
    assert_eq!(checker.infer_type(&expr), Ok(QuantumType::Qubit));
}

/// Test parsing and type-checking a multi-qubit register program
#[test]
fn test_multi_qubit_register_program() {
    let mut checker = TypeChecker::new();
    
    // Declare a 5-qubit register
    let statements = vec![
        Statement::Declaration(Declaration::QregDecl {
            name: "qreg".to_string(),
            size: 5,
        }),
        Statement::Declaration(Declaration::CregDecl {
            name: "creg".to_string(),
            size: 5,
        }),
    ];
    
    assert!(checker.check_program(&statements).is_ok());
    
    // Verify types
    assert_eq!(checker.env().get("qreg"), Some(&QuantumType::Qreg(5)));
    assert_eq!(checker.env().get("creg"), Some(&QuantumType::Creg(5)));
    
    // Access individual qubits
    for i in 0..5 {
        let expr = Expr::QregIndex {
            qreg: "qreg".to_string(),
            index: i,
        };
        assert_eq!(checker.infer_type(&expr), Ok(QuantumType::Qubit));
    }
    
    // Access individual bits
    for i in 0..5 {
        let expr = Expr::CregIndex {
            creg: "creg".to_string(),
            index: i,
        };
        assert_eq!(checker.infer_type(&expr), Ok(QuantumType::Bit));
    }
}

/// Test type inference for parameter declarations
#[test]
fn test_parameter_type_inference() {
    let mut checker = TypeChecker::new();
    
    let statements = vec![
        Statement::Declaration(Declaration::QregDecl {
            name: "q".to_string(),
            size: 2,
        }),
        Statement::Declaration(Declaration::ParamDecl {
            name: "theta".to_string(),
        }),
        Statement::Declaration(Declaration::ParamDecl {
            name: "phi".to_string(),
        }),
        Statement::Expression(Expr::NumLit(1.57)),
        Statement::Expression(Expr::Var("theta".to_string())),
    ];
    
    assert!(checker.check_program(&statements).is_ok());
    
    // Verify parameter types
    assert_eq!(checker.env().get("theta"), Some(&QuantumType::Param));
    assert_eq!(checker.env().get("phi"), Some(&QuantumType::Param));
    
    // Numeric literals should be Param type
    assert_eq!(checker.infer_type(&Expr::NumLit(3.14)), Ok(QuantumType::Param));
}

/// Test type inference for integer variables
#[test]
fn test_integer_type_inference() {
    let mut checker = TypeChecker::new();
    
    let statements = vec![
        Statement::Declaration(Declaration::IntDecl {
            name: "count".to_string(),
        }),
        Statement::Expression(Expr::IntLit(42)),
        Statement::Expression(Expr::Var("count".to_string())),
    ];
    
    assert!(checker.check_program(&statements).is_ok());
    
    assert_eq!(checker.env().get("count"), Some(&QuantumType::Int));
    assert_eq!(checker.infer_type(&Expr::IntLit(100)), Ok(QuantumType::Int));
}

/// Test type checking with quantum circuit type
#[test]
fn test_circuit_type() {
    // Circuit type is available for future circuit composition
    let circuit_type = QuantumType::Circuit;
    assert_eq!(format!("{}", circuit_type), "circuit");
}

/// Test error handling for undefined variables
#[test]
fn test_undefined_variable_error() {
    let checker = TypeChecker::new();
    let expr = Expr::Var("undefined_var".to_string());
    
    let result = checker.infer_type(&expr);
    assert!(result.is_err());
    
    if let Err(e) = result {
        assert!(e.to_string().contains("undefined"));
    }
}

/// Test error handling for out-of-bounds qreg access
#[test]
fn test_qreg_bounds_checking() {
    let mut checker = TypeChecker::new();
    
    checker
        .declare(&Declaration::QregDecl {
            name: "small".to_string(),
            size: 2,
        })
        .unwrap();
    
    // Valid access
    let valid_expr = Expr::QregIndex {
        qreg: "small".to_string(),
        index: 1,
    };
    assert!(checker.infer_type(&valid_expr).is_ok());
    
    // Invalid access (out of bounds)
    let invalid_expr = Expr::QregIndex {
        qreg: "small".to_string(),
        index: 5,
    };
    let result = checker.infer_type(&invalid_expr);
    assert!(result.is_err());
    
    if let Err(e) = result {
        assert!(e.to_string().contains("invalid qreg index"));
    }
}

/// Test error handling for out-of-bounds creg access
#[test]
fn test_creg_bounds_checking() {
    let mut checker = TypeChecker::new();
    
    checker
        .declare(&Declaration::CregDecl {
            name: "c".to_string(),
            size: 3,
        })
        .unwrap();
    
    // Valid access
    let valid_expr = Expr::CregIndex {
        creg: "c".to_string(),
        index: 2,
    };
    assert!(checker.infer_type(&valid_expr).is_ok());
    
    // Invalid access (out of bounds)
    let invalid_expr = Expr::CregIndex {
        creg: "c".to_string(),
        index: 10,
    };
    let result = checker.infer_type(&invalid_expr);
    assert!(result.is_err());
}

/// Test duplicate declaration error
#[test]
fn test_duplicate_declaration_error() {
    let mut checker = TypeChecker::new();
    
    let decl = Declaration::QregDecl {
        name: "dup".to_string(),
        size: 2,
    };
    
    assert!(checker.declare(&decl).is_ok());
    
    let result = checker.declare(&decl);
    assert!(result.is_err());
    
    if let Err(e) = result {
        assert!(e.to_string().contains("duplicate"));
    }
}

/// Test a complete example program with multiple quantum data types
#[test]
fn test_complete_quantum_program() {
    let mut checker = TypeChecker::new();
    
    // Simulate a complete quantum program
    let program = vec![
        // Declare quantum registers
        Statement::Declaration(Declaration::QregDecl {
            name: "q".to_string(),
            size: 3,
        }),
        Statement::Declaration(Declaration::CregDecl {
            name: "c".to_string(),
            size: 3,
        }),
        // Declare parameters for rotations
        Statement::Declaration(Declaration::ParamDecl {
            name: "alpha".to_string(),
        }),
        Statement::Declaration(Declaration::ParamDecl {
            name: "beta".to_string(),
        }),
        // Declare loop counter
        Statement::Declaration(Declaration::IntDecl {
            name: "i".to_string(),
        }),
        // Use the variables
        Statement::Expression(Expr::QregIndex {
            qreg: "q".to_string(),
            index: 0,
        }),
        Statement::Expression(Expr::QregIndex {
            qreg: "q".to_string(),
            index: 1,
        }),
        Statement::Expression(Expr::CregIndex {
            creg: "c".to_string(),
            index: 0,
        }),
        Statement::Expression(Expr::Var("alpha".to_string())),
        Statement::Expression(Expr::NumLit(0.5)),
        Statement::Expression(Expr::IntLit(0)),
    ];
    
    // Type check the entire program
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "Type checking failed: {:?}", result.err());
    
    // Verify all types were inferred correctly
    assert_eq!(checker.env().get("q"), Some(&QuantumType::Qreg(3)));
    assert_eq!(checker.env().get("c"), Some(&QuantumType::Creg(3)));
    assert_eq!(checker.env().get("alpha"), Some(&QuantumType::Param));
    assert_eq!(checker.env().get("beta"), Some(&QuantumType::Param));
    assert_eq!(checker.env().get("i"), Some(&QuantumType::Int));
}

/// Test type inference for qubit literals
#[test]
fn test_qubit_literal_inference() {
    let checker = TypeChecker::new();
    
    for i in 0..10 {
        let expr = Expr::QubitLit(i);
        assert_eq!(checker.infer_type(&expr), Ok(QuantumType::Qubit));
    }
}
