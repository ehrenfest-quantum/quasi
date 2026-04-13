// SPDX-License-Identifier: AGPL-3.0-or-later
//! Molecule geometry lookup.
//!
//! For the MVP, common molecules have hardcoded experimental geometries.
//! Complex molecules are imported from Garm JSON or explicit coordinate input.
//! Coordinates are in **bohr** (atomic units).

use crate::error::BridgeError;

/// An atom: (atomic_number, [x, y, z] in bohr).
pub type Atom = (u8, [f64; 3]);

/// Angstroms to bohr conversion factor.
const ANGSTROM_TO_BOHR: f64 = 1.8897259886;

/// Look up a molecule from a SMILES string.
///
/// Returns a list of atoms with 3D coordinates (bohr).
/// For the MVP, supports a hardcoded set of common molecules.
pub fn from_smiles(smiles: &str) -> Result<Vec<Atom>, BridgeError> {
    let canonical = smiles.trim();
    match canonical {
        // H2: bond length 0.7414 Å = 1.4 bohr
        "[H][H]" | "[HH]" => Ok(vec![
            (1, [0.0, 0.0, 0.0]),
            (1, [0.0, 0.0, 1.4]),
        ]),
        // Water: O-H 0.9578 Å, H-O-H 104.52°
        "O" => Ok(water_geometry()),
        // Methane (for future use)
        "C" => Ok(methane_geometry()),
        // HeH+ (minimal test system, 2 electrons)
        "[He][H+]" => Ok(vec![
            (2, [0.0, 0.0, 0.0]),
            (1, [0.0, 0.0, 1.4632]),
        ]),
        _ => Err(BridgeError::UnknownMolecule(format!(
            "no hardcoded geometry for SMILES '{}'; provide coordinates via JSON",
            canonical,
        ))),
    }
}

/// Count total electrons for a neutral molecule.
pub fn count_electrons(atoms: &[Atom]) -> usize {
    atoms.iter().map(|(z, _)| *z as usize).sum()
}

/// Element symbol from atomic number.
pub fn element_symbol(z: u8) -> &'static str {
    match z {
        1 => "H",
        2 => "He",
        6 => "C",
        7 => "N",
        8 => "O",
        9 => "F",
        _ => "??",
    }
}

/// Water geometry (bohr). O-H = 1.8094 bohr, angle 104.52°.
fn water_geometry() -> Vec<Atom> {
    let r_oh = 0.9578 * ANGSTROM_TO_BOHR; // 1.8094 bohr
    let angle = 104.52_f64.to_radians();
    let half_angle = angle / 2.0;
    vec![
        (8, [0.0, 0.0, 0.0]),
        (1, [0.0, r_oh * half_angle.sin(), r_oh * half_angle.cos()]),
        (1, [0.0, -r_oh * half_angle.sin(), r_oh * half_angle.cos()]),
    ]
}

/// Methane geometry (bohr). Tetrahedral, C-H = 1.089 Å.
fn methane_geometry() -> Vec<Atom> {
    let r_ch = 1.089 * ANGSTROM_TO_BOHR;
    // Tetrahedral unit vectors
    let a = r_ch / 3.0_f64.sqrt();
    vec![
        (6, [0.0, 0.0, 0.0]),
        (1, [a, a, a]),
        (1, [a, -a, -a]),
        (1, [-a, a, -a]),
        (1, [-a, -a, a]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h2_geometry() {
        let atoms = from_smiles("[H][H]").unwrap();
        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0].0, 1);
        assert_eq!(atoms[1].0, 1);
        let dist = (atoms[1].1[2] - atoms[0].1[2]).abs();
        assert!((dist - 1.4).abs() < 1e-10);
    }

    #[test]
    fn water_geometry_test() {
        let atoms = from_smiles("O").unwrap();
        assert_eq!(atoms.len(), 3);
        assert_eq!(atoms[0].0, 8); // oxygen
        assert_eq!(atoms[1].0, 1); // hydrogen
        assert_eq!(atoms[2].0, 1);
    }

    #[test]
    fn electron_count() {
        let h2 = from_smiles("[H][H]").unwrap();
        assert_eq!(count_electrons(&h2), 2);

        let water = from_smiles("O").unwrap();
        assert_eq!(count_electrons(&water), 10);
    }

    #[test]
    fn unknown_smiles_errors() {
        assert!(from_smiles("CC(=O)Oc1ccccc1C(=O)O").is_err());
    }
}
