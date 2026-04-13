// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! quasi-benchmark — GPCR quantum chemistry benchmark.
//!
//! Reproduces the quantum chemistry component of the UCL/IQM/NVIDIA hybrid
//! biomolecular simulation of a G-protein-coupled receptor (GPCR) from
//! Bickley et al. (arXiv:2505.16796), then extends beyond QPU limits using
//! Huoma tensor-network simulation and commensurability analysis.
//!
//! ```text
//! Step 1: Build Zundel cation (H5O2+) Hamiltonian from first principles
//! Step 2: Reproduce IQM QExa20 result via Huoma exact diagonalisation
//! Step 3: Commensurability analysis — automated active-space validation
//! Step 4: Extend active space beyond QPU qubit limits
//! ```

pub mod gpcr;
pub mod report;
