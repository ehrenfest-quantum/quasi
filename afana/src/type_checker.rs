//! Ehrenfest type checker and type inference engine

use crate::types::{QuantumType, TypeEnv, TypeError};

/// Expression types in Ehrenfest
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Variable reference
    Var(String),
    /// Qubit register indexing: qreg[index]
    QregIndex { qreg: String, index: usize },
    /// Classical register indexing: creg[index]
    CregIndex { creg: String, index: usize },
    /// Numeric literal (parameter)
    NumLit(f64),
    /// Integer literal
    IntLit(i64),
    /// Qubit literal (qubit identifier)
    QubitLit(usize),
}

/// Declaration types
#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    /// Qubit register declaration: qreg name[size]
    QregDecl { name: String, size: usize },
    /// Classical register declaration: creg name[size]
    CregDecl { name: String, size: usize },
    /// Parameter declaration: param name
    ParamDecl { name: String },
    /// Integer variable declaration: int name
    IntDecl { name: String },
}

/// Statement types
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Declaration(Declaration),
    Expression(Expr),
}

/// Type checker for Ehrenfest programs
#[derive(Debug, Default)]
pub struct TypeChecker {
    env: TypeEnv,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnv::new(),
        }
    }

    /// Get the current type environment
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Process a declaration and add it to the type environment
    pub fn declare(&mut self, decl: &Declaration) -> Result<(), TypeError> {
        match decl {
            Declaration::QregDecl { name, size } => {
                if self.env.contains(name) {
                    return Err(TypeError::DuplicateDeclaration(name.clone()));
                }
                self.env.insert(name.clone(), QuantumType::Qreg(*size));
                Ok(())
            }
            Declaration::CregDecl { name, size } => {
                if self.env.contains(name) {
                    return Err(TypeError::DuplicateDeclaration(name.clone()));
                }
                self.env.insert(name.clone(), QuantumType::Creg(*size));
                Ok(())
            }
            Declaration::ParamDecl { name } => {
                if self.env.contains(name) {
                    return Err(TypeError::DuplicateDeclaration(name.clone()));
                }
                self.env.insert(name.clone(), QuantumType::Param);
                Ok(())
            }
            Declaration::IntDecl { name } => {
                if self.env.contains(name) {
                    return Err(TypeError::DuplicateDeclaration(name.clone()));
                }
                self.env.insert(name.clone(), QuantumType::Int);
                Ok(())
            }
        }
    }

    /// Infer the type of an expression
    pub fn infer_type(&self, expr: &Expr) -> Result<QuantumType, TypeError> {
        match expr {
            Expr::Var(name) => {
                self.env
                    .get(name)
                    .cloned()
                    .ok_or_else(|| TypeError::UndefinedVariable(name.clone()))
            }
            Expr::QregIndex { qreg, index } => {
                match self.env.get(qreg) {
                    Some(QuantumType::Qreg(size)) => {
                        if *index >= *size {
                            Err(TypeError::InvalidQregIndex {
                                qreg: qreg.clone(),
                                index: *index,
                                size: *size,
                            })
                        } else {
                            Ok(QuantumType::Qubit)
                        }
                    }
                    Some(ty) => Err(TypeError::TypeMismatch {
                        expected: QuantumType::Qreg(0),
                        found: ty.clone(),
                    }),
                    None => Err(TypeError::UndefinedVariable(qreg.clone())),
                }
            }
            Expr::CregIndex { creg, index } => {
                match self.env.get(creg) {
                    Some(QuantumType::Creg(size)) => {
                        if *index >= *size {
                            Err(TypeError::InvalidCregIndex {
                                creg: creg.clone(),
                                index: *index,
                                size: *size,
                            })
                        } else {
                            Ok(QuantumType::Bit)
                        }
                    }
                    Some(ty) => Err(TypeError::TypeMismatch {
                        expected: QuantumType::Creg(0),
                        found: ty.clone(),
                    }),
                    None => Err(TypeError::UndefinedVariable(creg.clone())),
                }
            }
            Expr::NumLit(_) => Ok(QuantumType::Param),
            Expr::IntLit(_) => Ok(QuantumType::Int),
            Expr::QubitLit(_) => Ok(QuantumType::Qubit),
        }
    }

    /// Type check a statement
    pub fn check_statement(&mut self, stmt: &Statement) -> Result<(), TypeError> {
        match stmt {
            Statement::Declaration(decl) => self.declare(decl),
            Statement::Expression(expr) => {
                let _ty = self.infer_type(expr)?;
                Ok(())
            }
        }
    }

    /// Type check a sequence of statements
    pub fn check_program(&mut self, statements: &[Statement]) -> Result<(), TypeError> {
        for stmt in statements {
            self.check_statement(stmt)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_declare_qreg() {
        let mut checker = TypeChecker::new();
        let decl = Declaration::QregDecl {
            name: "q".to_string(),
            size: 5,
        };
        assert!(checker.declare(&decl).is_ok());
        assert_eq!(checker.env().get("q"), Some(&QuantumType::Qreg(5)));
    }

    #[test]
    fn test_declare_duplicate() {
        let mut checker = TypeChecker::new();
        let decl = Declaration::QregDecl {
            name: "q".to_string(),
            size: 5,
        };
        assert!(checker.declare(&decl).is_ok());
        assert!(checker.declare(&decl).is_err());
    }

    #[test]
    fn test_infer_qreg_index() {
        let mut checker = TypeChecker::new();
        checker
            .declare(&Declaration::QregDecl {
                name: "q".to_string(),
                size: 5,
            })
            .unwrap();

        let expr = Expr::QregIndex {
            qreg: "q".to_string(),
            index: 2,
        };
        assert_eq!(checker.infer_type(&expr), Ok(QuantumType::Qubit));
    }

    #[test]
    fn test_infer_invalid_qreg_index() {
        let mut checker = TypeChecker::new();
        checker
            .declare(&Declaration::QregDecl {
                name: "q".to_string(),
                size: 3,
            })
            .unwrap();

        let expr = Expr::QregIndex {
            qreg: "q".to_string(),
            index: 5,
        };
        assert!(checker.infer_type(&expr).is_err());
    }

    #[test]
    fn test_infer_undefined_variable() {
        let checker = TypeChecker::new();
        let expr = Expr::Var("undefined".to_string());
        assert!(checker.infer_type(&expr).is_err());
    }

    #[test]
    fn test_infer_literals() {
        let checker = TypeChecker::new();
        
        let num_expr = Expr::NumLit(3.14);
        assert_eq!(checker.infer_type(&num_expr), Ok(QuantumType::Param));
        
        let int_expr = Expr::IntLit(42);
        assert_eq!(checker.infer_type(&int_expr), Ok(QuantumType::Int));
        
        let qubit_expr = Expr::QubitLit(0);
        assert_eq!(checker.infer_type(&qubit_expr), Ok(QuantumType::Qubit));
    }

    #[test]
    fn test_check_program() {
        let mut checker = TypeChecker::new();
        let statements = vec![
            Statement::Declaration(Declaration::QregDecl {
                name: "q".to_string(),
                size: 3,
            }),
            Statement::Declaration(Declaration::CregDecl {
                name: "c".to_string(),
                size: 3,
            }),
            Statement::Declaration(Declaration::ParamDecl {
                name: "theta".to_string(),
            }),
            Statement::Expression(Expr::QregIndex {
                qreg: "q".to_string(),
                index: 0,
            }),
            Statement::Expression(Expr::Var("theta".to_string())),
        ];
        assert!(checker.check_program(&statements).is_ok());
    }

    #[test]
    fn test_type_mismatch() {
        let mut checker = TypeChecker::new();
        checker
            .declare(&Declaration::QregDecl {
                name: "q".to_string(),
                size: 3,
            })
            .unwrap();

        // Try to use qreg as a qubit directly (without indexing)
        let expr = Expr::Var("q".to_string());
        let result = checker.infer_type(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), QuantumType::Qreg(3));
    }
}
