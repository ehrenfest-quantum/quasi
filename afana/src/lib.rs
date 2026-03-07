// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Afana — the Ehrenfest-to-OpenQASM compiler.
//!
//! ```text
//! Ehrenfest (CBOR binary)
//!     → deserialize (cbor.rs)
//!     → EhrenfestProgram (Hamiltonians, observables, noise constraints)
//!     → trotterize (trotter.rs)
//!     → EhrenfestAst (gate sequences)
//!     → optimize (T-gate reduction, ZX-calculus)
//!     → emit OpenQASM 2.0 / 3.0
//! ```
//!
//! Ehrenfest programs are CBOR binary. There is no text form.

pub mod ast;
pub mod cbor;
pub mod emit;
pub mod error;
pub mod optimize;
pub mod trotter;
pub mod backend;
