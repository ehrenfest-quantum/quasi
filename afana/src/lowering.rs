// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Lowering pass for parametric Hamiltonian coefficients.
//!
//! This module converts parametric coefficient expressions from the Ehrenfest AST
//! into a symbolic representation in Afana IR, enabling compilation of variational
//! and time-dependent Hamiltonians for VQE and QAOA examples.

/// A coefficient in the Ehrenfest AST, which can be numeric or symbolic.
#[derive(Debug, Clone, PartialEq)]
pub enum Coefficient {
    /// A concrete numeric coefficient.
    Numeric(f64),
    /// A named parameter (e.g., variational angle "theta").
    Param(String),
    /// A symbolic expression (e.g., "a * cos(b*t)").
    Expr(String),
}

/// A symbolic coefficient in the Afana IR after lowering.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicCoefficient {
    /// A concrete numeric value.
    Numeric(f64),
    /// A bound parameter reference.
    Parameter(String),
    /// A symbolic expression string.
    Expression(String),
}

/// Lower a coefficient from the Ehrenfest AST to Afana IR symbolic representation.
///
/// This function handles all coefficient variants:
/// - `Coefficient::Numeric` → `SymbolicCoefficient::Numeric`
/// - `Coefficient::Param` → `SymbolicCoefficient::Parameter`
/// - `Coefficient::Expr` → `SymbolicCoefficient::Expression`
pub fn ast_to_ir(coeff: &Coefficient) -> SymbolicCoefficient {
    match coeff {
        Coefficient::Numeric(val) => SymbolicCoefficient::Numeric(*val),
        Coefficient::Param(name) => SymbolicCoefficient::Parameter(name.clone()),
        Coefficient::Expr(expr) => SymbolicCoefficient::Expression(expr.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_coefficient_lowers_to_numeric() {
        let coeff = Coefficient::Numeric(0.5);
        let ir = ast_to_ir(&coeff);
        assert_eq!(ir, SymbolicCoefficient::Numeric(0.5));
    }

    #[test]
    fn test_param_coefficient_lowers_to_parameter() {
        let coeff = Coefficient::Param("theta".to_string());
        let ir = ast_to_ir(&coeff);
        assert_eq!(ir, SymbolicCoefficient::Parameter("theta".to_string()));
    }

    #[test]
    fn test_expr_coefficient_lowers_to_expression() {
        let coeff = Coefficient::Expr("a * cos(b*t)".to_string());
        let ir = ast_to_ir(&coeff);
        assert_eq!(ir, SymbolicCoefficient::Expression("a * cos(b*t)".to_string()));
    }
}
