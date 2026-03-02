// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Afana — the Ehrenfest-to-OpenQASM compiler.
//!
//! ```text
//! Ehrenfest (.ef text / CBOR binary)
//!     → parse / deserialize
//!     → EhrenfestAst
//!     → optimize (T-gate reduction, ZX-calculus)
//!     → emit OpenQASM 2.0 / 3.0
//! ```

pub mod ast;
pub mod cbor;
pub mod emit;
pub mod error;
pub mod optimize;
pub mod parser;
