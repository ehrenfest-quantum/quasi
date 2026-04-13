// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zundel cation (H5O2+) Hamiltonian construction.
//!
//! The Zundel cation is the protonated water dimer — a shared proton
//! between two water molecules. It is the proof-of-concept system from
//! Bickley et al. (arXiv:2505.16796), used to validate the QSCI method
//! on the IQM QExa20 QPU before scaling to the full GPCR active site.
//!
//! Geometry: symmetric proton-sharing configuration with d(O-O) = 2.4 Å.
//! The shared proton sits at the midpoint of the O-O axis.
//!
//! Active space: selected via frontier partition centred on the HOMO-LUMO
//! gap. The proton-transfer coordinate (σ/σ* O-H-O) and surrounding
//! O lone pairs constitute the chemically relevant orbitals.
//!
//! STO-3G basis: 2 × O(5 functions) + 5 × H(1 function) = 15 basis functions
//!             = 15 spatial orbitals → 30 spin-orbitals (qubits) at full space.
//! Active-space selection reduces this to fit within exact-diag limits.

use quasi_bridge::basis;
use quasi_bridge::error::BridgeError;
use quasi_bridge::integrals::{self, MolecularIntegrals};
use quasi_bridge::jordan_wigner;
use quasi_bridge::molecule::Atom;
use quasi_bridge::partition;
use quasi_bridge::rhf::{self, RhfResult};

/// Ångström to bohr conversion.
const ANGSTROM_TO_BOHR: f64 = 1.8897259886;

/// Build the Zundel cation geometry (H5O2+) in bohr.
///
/// Symmetric configuration: two water molecules sharing a proton.
/// The O-O axis lies along z, with the shared proton at the origin.
/// d(O-O) = 2.4 Å (experimental value for the Zundel cation).
/// The terminal O-H bonds use water-like geometry (0.96 Å, 104.5° angle).
pub fn zundel_geometry() -> Vec<Atom> {
    let d_oo = 2.4 * ANGSTROM_TO_BOHR; // O-O distance
    let half_oo = d_oo / 2.0;

    // Terminal O-H bond parameters (water-like)
    let r_oh = 0.96 * ANGSTROM_TO_BOHR;
    let angle = 104.5_f64.to_radians();
    let half_angle = angle / 2.0;

    vec![
        // O1 at -z
        (8, [0.0, 0.0, -half_oo]),
        // H1a — terminal H on O1
        (1, [0.0, r_oh * half_angle.sin(), -half_oo - r_oh * half_angle.cos()]),
        // H1b — terminal H on O1
        (1, [0.0, -r_oh * half_angle.sin(), -half_oo - r_oh * half_angle.cos()]),
        // H_shared — bridging proton at origin
        (1, [0.0, 0.0, 0.0]),
        // O2 at +z
        (8, [0.0, 0.0, half_oo]),
        // H2a — terminal H on O2 (rotated 90° around z for symmetry breaking)
        (1, [r_oh * half_angle.sin(), 0.0, half_oo + r_oh * half_angle.cos()]),
        // H2b — terminal H on O2
        (1, [-r_oh * half_angle.sin(), 0.0, half_oo + r_oh * half_angle.cos()]),
    ]
}

/// Count electrons in the Zundel cation.
///
/// H5O2+ has 2×8 + 5×1 = 21 electrons as neutral, minus 1 for the cation = 20.
pub fn zundel_n_electrons() -> usize {
    20
}

/// Result of Zundel Hamiltonian construction.
pub struct ZundelHamiltonian {
    /// Pauli terms (coefficient, Pauli string).
    pub pauli_terms: Vec<(f64, String)>,
    /// Number of qubits (2 × active orbitals).
    pub n_qubits: usize,
    /// Number of active spatial orbitals.
    pub n_active: usize,
    /// RHF energy for reference.
    pub rhf_energy: f64,
    /// Nuclear repulsion energy.
    pub nuclear_repulsion: f64,
    /// Active orbital indices.
    pub active_orbitals: Vec<usize>,
    /// Total number of spatial orbitals (basis functions).
    pub n_spatial: usize,
}

/// Build the Zundel Hamiltonian with a given active space size.
///
/// The full STO-3G basis gives 15 spatial orbitals (30 qubits). Since
/// exact diagonalisation is limited to 20 qubits, we select an active
/// space of `n_active_orbitals` centred on the HOMO-LUMO gap.
///
/// Typical values:
///   - n_active_orbitals = 4  →  8 qubits (fast validation)
///   - n_active_orbitals = 6  → 12 qubits
///   - n_active_orbitals = 8  → 16 qubits
///   - n_active_orbitals = 10 → 20 qubits (matches IQM QExa20)
pub fn build_zundel_hamiltonian(
    n_active_orbitals: Option<usize>,
) -> Result<ZundelHamiltonian, BridgeError> {
    let atoms = zundel_geometry();
    let n_electrons = zundel_n_electrons();

    // Build basis and compute integrals
    let bf = basis::build_basis(&atoms, "sto-3g")?;
    let n_spatial = bf.len();
    let ints = integrals::compute_integrals(&atoms, &bf);

    // RHF — closed shell (20 electrons)
    let rhf_result = rhf::rhf(&ints, n_electrons)?;

    // Transform integrals to MO basis
    let h_core_ao: Vec<Vec<f64>> = (0..n_spatial)
        .map(|i| {
            (0..n_spatial)
                .map(|j| ints.kinetic[i][j] + ints.nuclear[i][j])
                .collect()
        })
        .collect();
    let h_mo = rhf::ao_to_mo_one(&h_core_ao, &rhf_result.mo_coefficients);
    let eri_mo = rhf::ao_to_mo_two(&ints, &rhf_result.mo_coefficients);

    // Active-space selection
    let part = partition::frontier_partition(n_spatial, n_electrons, n_active_orbitals)?;
    let n_active = part.active.len();
    let active_orbitals = part.active.clone();

    // Select active-space integrals
    let (h_act, eri_act, na) =
        partition::select_active_integrals(&h_mo, &eri_mo, n_spatial, &part);

    // Jordan-Wigner transform
    let pauli_terms =
        jordan_wigner::jordan_wigner_transform(&h_act, &eri_act, na, ints.nuclear_repulsion);
    let n_qubits = 2 * na;

    Ok(ZundelHamiltonian {
        pauli_terms,
        n_qubits,
        n_active,
        rhf_energy: rhf_result.energy,
        nuclear_repulsion: ints.nuclear_repulsion,
        active_orbitals,
        n_spatial,
    })
}

/// Run the full pipeline on a known molecule to validate the integral engine.
///
/// Knuth rules: validate against textbook values for simpler systems before
/// running on Zundel. Returns (computed_energy, reference_energy, molecule_name).
pub fn validate_h2() -> Result<(f64, f64, &'static str), BridgeError> {
    // H2 at R = 1.4 bohr, STO-3G
    // Reference FCI/STO-3G: -1.1373 Ha (Szabo & Ostlund)
    let atoms: Vec<Atom> = vec![
        (1, [0.0, 0.0, 0.0]),
        (1, [0.0, 0.0, 1.4]),
    ];
    let energy = compute_energy_from_atoms(&atoms, 2, None)?;
    Ok((energy, -1.1373, "H2"))
}

/// Validate with water (H2O).
pub fn validate_water() -> Result<(f64, f64, &'static str), BridgeError> {
    // Water, STO-3G
    // Reference RHF/STO-3G: ~-74.96 Ha, FCI/STO-3G: ~-75.01 Ha
    let atoms = quasi_bridge::molecule::from_smiles("O")?;
    let n_electrons = quasi_bridge::molecule::count_electrons(&atoms);
    // Use 4-orbital active space to keep within exact diag limit
    // (full 7 orbitals = 14 qubits, also fine)
    let energy = compute_energy_from_atoms(&atoms, n_electrons, Some(4))?;
    // With a 4-orbital active space, we capture partial correlation
    Ok((energy, -75.01, "H2O"))
}

/// Compute ground-state energy from atom coordinates via the full pipeline.
fn compute_energy_from_atoms(
    atoms: &[Atom],
    n_electrons: usize,
    n_active_orbitals: Option<usize>,
) -> Result<f64, BridgeError> {
    let bf = basis::build_basis(atoms, "sto-3g")?;
    let n_spatial = bf.len();
    let ints = integrals::compute_integrals(atoms, &bf);
    let rhf_result = rhf::rhf(&ints, n_electrons)?;

    let h_core_ao: Vec<Vec<f64>> = (0..n_spatial)
        .map(|i| {
            (0..n_spatial)
                .map(|j| ints.kinetic[i][j] + ints.nuclear[i][j])
                .collect()
        })
        .collect();
    let h_mo = rhf::ao_to_mo_one(&h_core_ao, &rhf_result.mo_coefficients);
    let eri_mo = rhf::ao_to_mo_two(&ints, &rhf_result.mo_coefficients);

    let part = partition::frontier_partition(n_spatial, n_electrons, n_active_orbitals)?;
    let (h_act, eri_act, na) =
        partition::select_active_integrals(&h_mo, &eri_mo, n_spatial, &part);

    let pauli_terms =
        jordan_wigner::jordan_wigner_transform(&h_act, &eri_act, na, ints.nuclear_repulsion);
    let n_qubits = 2 * na;

    quasi_bridge::postprocess::ground_state_energy(&pauli_terms, n_qubits)
}

/// Get raw molecular integrals and RHF result for the Zundel cation.
///
/// Used by the commensurability and extension modules that need access
/// to the intermediate data (not just the final Pauli Hamiltonian).
pub fn zundel_integrals() -> Result<(MolecularIntegrals, RhfResult, usize), BridgeError> {
    let atoms = zundel_geometry();
    let n_electrons = zundel_n_electrons();
    let bf = basis::build_basis(&atoms, "sto-3g")?;
    let n_spatial = bf.len();
    let ints = integrals::compute_integrals(&atoms, &bf);
    let rhf_result = rhf::rhf(&ints, n_electrons)?;
    Ok((ints, rhf_result, n_spatial))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zundel_geometry_has_7_atoms() {
        let atoms = zundel_geometry();
        assert_eq!(atoms.len(), 7);
        // 2 oxygens, 5 hydrogens
        let n_o = atoms.iter().filter(|(z, _)| *z == 8).count();
        let n_h = atoms.iter().filter(|(z, _)| *z == 1).count();
        assert_eq!(n_o, 2);
        assert_eq!(n_h, 5);
    }

    #[test]
    fn zundel_electron_count() {
        assert_eq!(zundel_n_electrons(), 20);
    }

    #[test]
    fn zundel_basis_has_15_functions() {
        let atoms = zundel_geometry();
        let bf = basis::build_basis(&atoms, "sto-3g").unwrap();
        // 2 × O(5) + 5 × H(1) = 15
        assert_eq!(bf.len(), 15);
    }

    #[test]
    fn validate_h2_energy() {
        let (energy, reference, name) = validate_h2().unwrap();
        let delta = (energy - reference).abs();
        assert!(
            delta < 0.05,
            "{name}: computed {energy:.6} vs reference {reference:.6}, delta {delta:.4}"
        );
    }

    #[test]
    fn build_zundel_small_active_space() {
        // 4-orbital active space → 8 qubits (fast test)
        let ham = build_zundel_hamiltonian(Some(4)).unwrap();
        assert_eq!(ham.n_qubits, 8);
        assert_eq!(ham.n_active, 4);
        assert!(!ham.pauli_terms.is_empty());
        assert_eq!(ham.n_spatial, 15);
    }
}
