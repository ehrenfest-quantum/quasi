// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! quasi-bridge — Garm -> QUASI molecular pipeline.
//!
//! Closes the gap between Garm's chemistry reconnaissance and QUASI's
//! quantum compilation stack.  A SMILES string goes in; a ground-state
//! energy comes out, having passed through every QUASI layer:
//!
//! ```text
//! SMILES -> molecule -> integrals -> RHF -> Jordan-Wigner
//!        -> Ehrenfest CBOR -> Afana -> Solvayeur -> execute -> energy
//! ```

pub mod basis;
pub mod ehrenfest_mol;
pub mod error;
pub mod integrals;
pub mod jordan_wigner;
pub mod molecule;
pub mod partition;
pub mod pipeline;
pub mod postprocess;
pub mod rhf;
pub mod server;
