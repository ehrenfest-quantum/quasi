// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Typed AST for Ehrenfest programs.
//!
//! Produced by the Trotterization pass (`trotter.rs`) from the physics-level
//! `EhrenfestProgram` (deserialized from CBOR by `cbor.rs`). The AST is the
//! circuit-level representation consumed by the QASM emitter and ZX optimizer.

use serde::{Deserialize, Serialize};

// ── Gate names ───────────────────────────────────────────────────────────────

/// The set of gates Afana recognises.
///
/// Stored as lower-case canonical names. Aliases (`cnot` → `cx`,
/// `toffoli` → `ccx`) are resolved at parse time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GateName {
    Rz,
    H,
    X,
    Y,
    Z,
    S,
    T,
    Sdg,
    Tdg,
    Cx,
    Cz,
    Swap,
    Ccx,
    Rx,
    Ry,
    Rz,
}

impl GateName {
    /// Parse a gate name from a lower-cased token.
    ///
    /// Returns `None` for unrecognised tokens.
    pub fn from_token(tok: &str) -> Option<Self> {
        match tok {
            "h" => Some(Self::H),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            "s" => Some(Self::S),
            "t" => Some(Self::T),
            "sdg" => Some(Self::Sdg),
            "tdg" => Some(Self::Tdg),
            "cx" | "cnot" => Some(Self::Cx),
            "cz" => Some(Self::Cz),
            "swap" => Some(Self::Swap),
            "ccx" | "toffoli" => Some(Self::Ccx),
            "rx" => Some(Self::Rx),
            "ry" => Some(Self::Ry),
            "rz" => Some(Self::Rz),
            _ => None,
        }
    }

    /// Canonical lower-case QASM name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::H => "h",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::S => "s",
            Self::T => "t",
            Self::Sdg => "sdg",
            Self::Tdg => "tdg",
            Self::Cx => "cx",
            Self::Cz => "cz",
            Self::Swap => "swap",
            Self::Ccx => "ccx",
            Self::Rx => "rx",
            Self::Ry => "ry",
            Self::Rz => "rz",
        }
    }

    /// Number of qubit operands this gate requires.
    pub fn arity(&self) -> usize {
        match self {
            Self::H | Self::X | Self::Y | Self::Z => 1,
            Self::S | Self::T | Self::Sdg | Self::Tdg => 1,
            Self::Rx | Self::Ry | Self::Rz => 1,
            Self::Cx | Self::Cz | Self::Swap => 2,
            Self::Ccx => 3,
        }
    }

    /// Whether this gate takes a rotation parameter.
    pub fn is_parametric(&self) -> bool {
        matches!(self, Self::Rx | Self::Ry | Self::Rz)
    }
}

impl std::fmt::Display for GateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── AST nodes ────────────────────────────────────────────────────────────────

/// A single gate application, e.g. `h q0` or `rx 1.57 q0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    pub name: GateName,
    pub qubits: Vec<usize>,
    pub params: Vec<f64>,
}

/// A measurement directive: `measure qN -> cN`.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    pub qubit: usize,
    pub cbit: usize,
}

/// A classically-conditioned gate: `if cN == M: gate qN`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalGate {
    pub cbit: usize,
    pub cbit_value: u32,
    pub gate: Gate,
}

/// An assertion hint (non-executable): `expect state "..."`.
#[derive(Debug, Clone, PartialEq)]
pub struct Expect {
    pub kind: ExpectKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectKind {
    State,
    Counts,
    Relation,
}

/// A user-defined type alias: `type QubitPair = (Qubit, Qubit)`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name: String,
    pub definition: String,
}

/// A gate inside a variational loop with symbolic parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct VariationalGate {
    pub name: GateName,
    pub qubits: Vec<usize>,
    pub param_refs: Vec<String>,
}

/// A variational optimisation block (VQE/QAOA ansatz).
#[derive(Debug, Clone, PartialEq)]
pub struct VariationalLoop {
    pub params: Vec<String>,
    pub max_iter: u32,
    pub body: Vec<VariationalGate>,
}

/// Root AST node for a parsed Ehrenfest program.
#[derive(Debug, Clone, PartialEq)]
pub struct EhrenfestAst {
    pub name: String,
    pub n_qubits: usize,
    pub prepare: Option<String>,
    pub gates: Vec<Gate>,
    pub measures: Vec<Measure>,
    pub conditionals: Vec<ConditionalGate>,
    pub expects: Vec<Expect>,
    pub type_decls: Vec<TypeDecl>,
    pub variational_loops: Vec<VariationalLoop>,
}
