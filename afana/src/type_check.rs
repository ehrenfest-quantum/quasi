use crate::ast::{EhrenfestAst, GateName};
use crate::error::TypeError;

/// Perform semantic analysis and type checking on the AST.
pub fn check(ast: &EhrenfestAst) -> Result<(), TypeError> {
    let mut checker = TypeChecker::new(ast.n_qubits);
    checker.check_ast(ast)
}

struct TypeChecker {
    n_qubits: usize,
}

impl TypeChecker {
    fn new(n_qubits: usize) -> Self {
        Self { n_qubits }
    }

    fn check_ast(&mut self, ast: &EhrenfestAst) -> Result<(), TypeError> {
        // Check all gates
        for gate in &ast.gates {
            self.check_gate(gate)?;
        }

        // Check conditional gates
        for cond in &ast.conditionals {
            self.check_gate(&cond.gate)?;
        }

        // Check variational loops
        for vloop in &ast.variational_loops {
            for vg in &vloop.body {
                self.check_variational_gate(vg)?;
            }
        }

        Ok(())
    }

    fn check_gate(&self, gate: &crate::ast::Gate) -> Result<(), TypeError> {
        // Check qubit count matches gate arity
        if gate.qubits.len() != gate.name.arity() {
            return Err(TypeError::ArityMismatch {
                gate: gate.name.to_string(),
                expected: gate.name.arity(),
                found: gate.qubits.len(),
            });
        }

        // Check all qubits are in range
        for &qubit in &gate.qubits {
            if qubit >= self.n_qubits {
                return Err(TypeError::QubitOutOfRange {
                    qubit,
                    max: self.n_qubits - 1,
                });
            }
        }

        // Check parameter count
        let expected_params = if gate.name.is_parametric() { 1 } else { 0 };
        if gate.params.len() != expected_params {
            return Err(TypeError::ParameterMismatch {
                gate: gate.name.to_string(),
                expected: expected_params,
                found: gate.params.len(),
            });
        }

        Ok(())
    }

    fn check_variational_gate(&self, gate: &crate::ast::VariationalGate) -> Result<(), TypeError> {
        // Check qubit count matches gate arity
        if gate.qubits.len() != gate.name.arity() {
            return Err(TypeError::ArityMismatch {
                gate: gate.name.to_string(),
                expected: gate.name.arity(),
                found: gate.qubits.len(),
            });
        }

        // Check all qubits are in range
        for &qubit in &gate.qubits {
            if qubit >= self.n_qubits {
                return Err(TypeError::QubitOutOfRange {
                    qubit,
                    max: self.n_qubits - 1,
                });
            }
        }

        // Check parametric gates have parameters
        if gate.name.is_parametric() && gate.param_refs.is_empty() {
            return Err(TypeError::ParameterRequired(gate.name.to_string()));
        }

        // Non-parametric gates should not have parameters
        if !gate.name.is_parametric() && !gate.param_refs.is_empty() {
            return Err(TypeError::UnexpectedParameters(gate.name.to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn test_valid_circuit() {
        let source = r#"
program \"test\"
qubits 2
h q0
cnot q0 q1
        "#;
        let ast = parser::parse(source).unwrap();
        assert!(check(&ast).is_ok());
    }

    #[test]
    fn test_gate_arity_mismatch() {
        let source = r#"
program \"test\"
qubits 2
h q0 q1
        "#;
        let ast = parser::parse(source).unwrap();
        let result = check(&ast);
        assert!(matches!(result, Err(TypeError::ArityMismatch { .. })));
    }

    #[test]
    fn test_qubit_out_of_range() {
        let source = r#"
program \"test\"
qubits 2
h q5
        "#;
        let ast = parser::parse(source).unwrap();
        let result = check(&ast);
        assert!(matches!(result, Err(TypeError::QubitOutOfRange { .. })));
    }

    #[test]
    fn test_parameter_mismatch() {
        let source = r#"
program \"test\"
qubits 1
rx q0
        "#;
        let ast = parser::parse(source).unwrap();
        let result = check(&ast);
        assert!(matches!(result, Err(TypeError::ParameterMismatch { .. })));
    }

    #[test]
    fn test_variational_parameter_required() {
        let source = r#"
program \"test\"
qubits 1
variational params theta
  rx q0
end
        "#;
        let ast = parser::parse(source).unwrap();
        let result = check(&ast);
        assert!(matches!(result, Err(TypeError::ParameterRequired(_))));
    }
}