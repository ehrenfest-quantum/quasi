// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! quasi-bridge — chemistry primitives library.
//!
//! Library-only crate exposing the building blocks of a SMILES → qubit
//! Hamiltonian → ground-state energy pipeline:
//!
//! - [`molecule`] — minimal SMILES → 3D atoms (toy molecules).
//! - [`basis`], [`integrals`] — STO-3G integrals.
//! - [`rhf`] — restricted Hartree–Fock solver.
//! - [`partition`] — frontier-orbital active-space selection.
//! - [`jordan_wigner`] — second-quantised → Pauli Hamiltonian.
//! - [`ehrenfest_mol`] — Pauli terms → Afana-compatible CBOR program.
//! - [`postprocess`] — exact diagonalisation of small Pauli Hamiltonians.
//!
//! The HTTP server (`/run`, `/analyze`, `/run_active_space`), the CLI
//! (`quasi-bridge run …`), and the orchestration that wove these
//! primitives together (`pipeline::run_molecule`) were removed in the
//! consolidation cleanup (2026-05-07): the SMILES → energy workflow now
//! lives as the `smiles_energy` kata in Arn, and the modules in this
//! crate are consumed directly by `quasi-cudaq-ffi`.
//!
//! See `docs/CONSOLIDATION_ROADMAP_2026_05.md` in
//! `hiq-lab/valiant-garm-platform` for the full rationale.

pub mod basis;
pub mod ehrenfest_mol;
pub mod error;
pub mod integrals;
pub mod jordan_wigner;
pub mod molecule;
pub mod partition;
pub mod postprocess;
pub mod rhf;
