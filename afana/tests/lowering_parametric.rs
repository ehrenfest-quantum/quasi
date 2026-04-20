// Integration tests for parametric coefficient lowering.

use afana::lowering::{ast_to_ir, Coefficient, SymbolicCoefficient};

#[test]
fn lowering_param_coefficient_yields_symbolic_parameter() {
    // Verify that lowering a Hamiltonian with coefficient Coefficient::Param("theta")
    // yields an Afana IR term with SymbolicCoefficient::Parameter("theta").
    let coeff = Coefficient::Param("theta".to_string());
    let ir = ast_to_ir(&coeff);
    assert_eq!(ir, SymbolicCoefficient::Parameter("theta".to_string()));
}

#[test]
fn lowering_expr_coefficient_yields_symbolic_expression() {
    // Verify that Coefficient::Expr variants are handled correctly.
    let coeff = Coefficient::Expr("a * cos(b*t)".to_string());
    let ir = ast_to_ir(&coeff);
    assert_eq!(ir, SymbolicCoefficient::Expression("a * cos(b*t)".to_string()));
}

#[test]
fn lowering_numeric_coefficient_preserves_value() {
    // Verify that numeric coefficients pass through unchanged.
    let coeff = Coefficient::Numeric(0.5);
    let ir = ast_to_ir(&coeff);
    assert_eq!(ir, SymbolicCoefficient::Numeric(0.5));
}

#[test]
fn lowering_handles_all_coefficient_variants() {
    // Comprehensive test that ast_to_ir handles all Coefficient variants.
    let test_cases = vec![
        (Coefficient::Numeric(1.0), SymbolicCoefficient::Numeric(1.0)),
        (Coefficient::Numeric(-0.5), SymbolicCoefficient::Numeric(-0.5)),
        (Coefficient::Param("theta".to_string()), SymbolicCoefficient::Parameter("theta".to_string())),
        (Coefficient::Param("phi".to_string()), SymbolicCoefficient::Parameter("phi".to_string())),
        (Coefficient::Expr("cos(t)".to_string()), SymbolicCoefficient::Expression("cos(t)".to_string())),
        (Coefficient::Expr("a + b".to_string()), SymbolicCoefficient::Expression("a + b".to_string())),
    ];

    for (input, expected) in test_cases {
        assert_eq!(ast_to_ir(&input), expected, "Failed for input: {:?}", input);
    }
}
