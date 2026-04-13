// SPDX-License-Identifier: AGPL-3.0-or-later
//! Full GPCR active site benchmark (future work).
//!
//! For the full GPCR active site (β2-adrenergic receptor, PDB: 2RH1),
//! two paths exist:
//!
//! **Path A: Published Hamiltonian data**
//! If the Bickley et al. supplementary data includes Pauli coefficients
//! for the GPCR active-site Hamiltonian, import them directly.
//!
//! **Path B: Build from PDB structure**
//! 1. Extract active-site coordinates from PDB 2RH1
//! 2. Use Garm Analysis to identify the quantum-amenable region
//!    (metal coordination, multi-reference character)
//! 3. Build molecular Hamiltonian via quasi-bridge
//! 4. Select active space via sin(C/2) — automated, no human input
//! 5. Run on Huoma: exact diag for small, MPS for medium, ProjectedTTN
//!    for large active spaces
//!
//! This module is a placeholder for the full GPCR pipeline. The Zundel
//! cation serves as the proof-of-concept; the full GPCR is the target.
//!
//! Requirements for Path B:
//! - PDB parser (or use Garm's existing PDB support)
//! - QM/MM-like embedding (extract quantum region from protein)
//! - Larger basis set support (cc-pVDZ) in quasi-bridge
//! - MPS and ProjectedTTN solvers in Huoma
//!
//! These capabilities are under development. The Zundel benchmark
//! validates the pipeline for when they are ready.

/// Placeholder for the full GPCR benchmark.
///
/// Returns a description of what this benchmark will do when the
/// required capabilities (PDB import, larger basis sets, MPS solver)
/// are available.
pub fn describe() -> &'static str {
    "Full GPCR active site benchmark (β2-adrenergic receptor, PDB: 2RH1)\n\
     \n\
     Status: awaiting MPS solver and cc-pVDZ basis in quasi-bridge.\n\
     \n\
     The Zundel cation benchmark validates the pipeline:\n\
     - Integral engine: validated against H2/STO-3G textbook values\n\
     - RHF solver: converges for 20-electron closed-shell system\n\
     - Jordan-Wigner: produces correct Pauli Hamiltonian\n\
     - Exact diag: matches FCI for small active spaces\n\
     - sin(C/2): identifies active orbitals automatically\n\
     \n\
     When the full GPCR pipeline is ready, the same code path applies\n\
     with larger coordinates and basis sets."
}
