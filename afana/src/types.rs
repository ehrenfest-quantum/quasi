//! Ehrenfest type system - quantum data types and type definitions

use std::fmt;

/// Core quantum data types supported by Ehrenfest
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QuantumType {
    /// Single qubit type
    Qubit,
    /// Quantum register (array of qubits)
    Qreg(usize),
    /// Quantum circuit type
    Circuit,
    /// Classical bit
    Bit,
    /// Classical register (array of bits)
    Creg(usize),
    /// Floating point parameter (for rotation angles, etc.)
    Param,
    /// Integer parameter
    Int,
}

impl fmt::Display for QuantumType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantumType::Qubit => write!(f, "qubit"),
            QuantumType::Qreg(size) => write!(f, "qreg[{}]", size),
            QuantumType::Circuit => write!(f, "circuit"),
            QuantumType::Bit => write!(f, "bit"),
            QuantumType::Creg(size) => write!(f, "creg[{}]", size),
            QuantumType::Param => write!(f, "param"),
            QuantumType::Int => write!(f, "int"),
        }
    }
}

/// Type environment for tracking variable types
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    variables: std::collections::HashMap<String, QuantumType>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            variables: std::collections::HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, ty: QuantumType) {
        self.variables.insert(name, ty);
    }

    pub fn get(&self, name: &str) -> Option<&QuantumType> {
        self.variables.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

/// Type checking errors
#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    UndefinedVariable(String),
    TypeMismatch { expected: QuantumType, found: QuantumType },
    InvalidQregIndex { qreg: String, index: usize, size: usize },
    InvalidCregIndex { creg: String, index: usize, size: usize },
    DuplicateDeclaration(String),
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UndefinedVariable(name) => {
                write!(f, "undefined variable: {}", name)
            }
            TypeError::TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {}, found {}", expected, found)
            }
            TypeError::InvalidQregIndex { qreg, index, size } => {
                write!(f, "invalid qreg index: {}[{}] but size is {}", qreg, index, size)
            }
            TypeError::InvalidCregIndex { creg, index, size } => {
                write!(f, "invalid creg index: {}[{}] but size is {}", creg, index, size)
            }
            TypeError::DuplicateDeclaration(name) => {
                write!(f, "duplicate declaration: {}", name)
            }
        }
    }
}

impl std::error::Error for TypeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_type_display() {
        assert_eq!(format!("{}", QuantumType::Qubit), "qubit");
        assert_eq!(format!("{}", QuantumType::Qreg(5)), "qreg[5]");
        assert_eq!(format!("{}", QuantumType::Circuit), "circuit");
        assert_eq!(format!("{}", QuantumType::Bit), "bit");
        assert_eq!(format!("{}", QuantumType::Creg(3)), "creg[3]");
        assert_eq!(format!("{}", QuantumType::Param), "param");
        assert_eq!(format!("{}", QuantumType::Int), "int");
    }

    #[test]
    fn test_type_env_operations() {
        let mut env = TypeEnv::new();
        assert!(!env.contains("q"));
        
        env.insert("q".to_string(), QuantumType::Qubit);
        assert!(env.contains("q"));
        assert_eq!(env.get("q"), Some(&QuantumType::Qubit));
        
        env.insert("qr".to_string(), QuantumType::Qreg(5));
        assert_eq!(env.get("qr"), Some(&QuantumType::Qreg(5)));
    }

    #[test]
    fn test_type_error_display() {
        let err = TypeError::UndefinedVariable("x".to_string());
        assert!(err.to_string().contains("x"));
        
        let err = TypeError::TypeMismatch {
            expected: QuantumType::Qubit,
            found: QuantumType::Bit,
        };
        assert!(err.to_string().contains("qubit"));
        assert!(err.to_string().contains("bit"));
    }
}
