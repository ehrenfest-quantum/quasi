// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! QASM3 generation from ZX-IR.
//!
//! This module provides synthesis of QASM3 output from ZX-calculus
//! intermediate representation, including measurement gates with
//! classical register assignment.

pub mod generation;

pub use generation::*;
