// SPDX-License-Identifier: AGPL-3.0-or-later
//! Active-space partition for molecular Hamiltonians.
//!
//! Determines which orbitals need quantum treatment (active space) and
//! which can be treated classically (environment). Supports:
//! - Import from Garm Analysis JSON output
//! - Simple energy-based partition (frontier orbitals around HOMO-LUMO)

use crate::error::BridgeError;
use serde::{Deserialize, Serialize};

/// Active-space partition result.
#[derive(Debug, Clone)]
pub struct ActiveSpacePartition {
    /// Indices of spatial orbitals in the active space.
    pub active: Vec<usize>,
    /// Indices of orbitals in the frozen core + virtual environment.
    pub environment: Vec<usize>,
    /// Number of active electrons.
    pub n_electrons_active: usize,
    /// Method used for partition.
    pub method: String,
}

/// Garm Analysis output format (subset of fields we need).
#[derive(Debug, Deserialize, Serialize)]
pub struct GarmAnalysis {
    pub molecules: Vec<GarmMolecule>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GarmMolecule {
    pub smiles: String,
    #[serde(default)]
    pub active_orbitals: Vec<usize>,
    #[serde(default)]
    pub n_active_electrons: usize,
    #[serde(default)]
    pub qubit_estimate: usize,
    #[serde(default)]
    pub qpu_flag: bool,
    #[serde(default)]
    pub sinc2_threshold: f64,
}

/// Import a partition from Garm Analysis JSON.
pub fn from_garm_json(
    json: &str,
    n_spatial: usize,
    n_electrons: usize,
) -> Result<ActiveSpacePartition, BridgeError> {
    let analysis: GarmAnalysis =
        serde_json::from_str(json).map_err(|e| BridgeError::GarmParse(e.to_string()))?;
    let mol = analysis
        .molecules
        .first()
        .ok_or_else(|| BridgeError::GarmParse("no molecules in Garm output".into()))?;

    if mol.active_orbitals.is_empty() {
        // Fall back to frontier partition
        return frontier_partition(n_spatial, n_electrons, None);
    }

    let active = mol.active_orbitals.clone();
    let environment: Vec<usize> = (0..n_spatial).filter(|i| !active.contains(i)).collect();
    let n_active_elec = if mol.n_active_electrons > 0 {
        mol.n_active_electrons
    } else {
        active.len().min(n_electrons)
    };

    Ok(ActiveSpacePartition {
        active,
        environment,
        n_electrons_active: n_active_elec,
        method: format!("garm_sinc2(threshold={})", mol.sinc2_threshold),
    })
}

/// Simple frontier-orbital partition around the HOMO-LUMO gap.
///
/// Selects `n_active` orbitals centred on the Fermi level. If not
/// specified, defaults to all orbitals (full space).
pub fn frontier_partition(
    n_spatial: usize,
    n_electrons: usize,
    n_active: Option<usize>,
) -> Result<ActiveSpacePartition, BridgeError> {
    let n_occ = n_electrons / 2;
    let n_act = n_active.unwrap_or(n_spatial);

    if n_act > n_spatial {
        return Err(BridgeError::ActiveSpaceTooLarge {
            n_qubits: 2 * n_act,
            max_qubits: 2 * n_spatial,
        });
    }

    // Centre the window on the HOMO-LUMO gap
    let half = n_act / 2;
    let start = n_occ.saturating_sub(half);
    let end = (start + n_act).min(n_spatial);
    let start = end - n_act.min(end);

    let active: Vec<usize> = (start..end).collect();
    let environment: Vec<usize> = (0..n_spatial).filter(|i| !active.contains(i)).collect();

    // Count active electrons: 2 per doubly-occupied orbital in the active space
    let n_active_elec = active.iter().filter(|&&i| i < n_occ).count() * 2;

    Ok(ActiveSpacePartition {
        active,
        environment,
        n_electrons_active: n_active_elec,
        method: format!("frontier(n_active={})", n_act),
    })
}

/// Select active-space one- and two-electron integrals from full MO integrals.
pub fn select_active_integrals(
    h_mo: &[Vec<f64>],
    eri_mo: &[f64],
    n_spatial: usize,
    partition: &ActiveSpacePartition,
) -> (Vec<Vec<f64>>, Vec<f64>, usize) {
    let na = partition.active.len();

    let mut h_act = vec![vec![0.0; na]; na];
    for (i, &p) in partition.active.iter().enumerate() {
        for (j, &q) in partition.active.iter().enumerate() {
            h_act[i][j] = h_mo[p][q];
        }
    }

    let mut eri_act = vec![0.0; na * na * na * na];
    for (i, &p) in partition.active.iter().enumerate() {
        for (j, &q) in partition.active.iter().enumerate() {
            for (k, &r) in partition.active.iter().enumerate() {
                for (l, &s) in partition.active.iter().enumerate() {
                    eri_act[i * na * na * na + j * na * na + k * na + l] =
                        eri_mo[p * n_spatial * n_spatial * n_spatial
                            + q * n_spatial * n_spatial
                            + r * n_spatial
                            + s];
                }
            }
        }
    }

    (h_act, eri_act, na)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontier_full_space() {
        let part = frontier_partition(7, 10, None).unwrap();
        assert_eq!(part.active.len(), 7);
        assert!(part.environment.is_empty());
    }

    #[test]
    fn frontier_4_orbitals() {
        let part = frontier_partition(7, 10, Some(4)).unwrap();
        assert_eq!(part.active.len(), 4);
        assert_eq!(part.environment.len(), 3);
        // HOMO is orbital 4 (n_occ=5), so window should be centred there
        assert!(part.active.contains(&4));
        assert!(part.active.contains(&3));
    }

    #[test]
    fn garm_json_parse() {
        let json = r#"{"molecules":[{"smiles":"O","active_orbitals":[3,4,5,6],"n_active_electrons":4,"qubit_estimate":8,"qpu_flag":false,"sinc2_threshold":0.3}]}"#;
        let part = from_garm_json(json, 7, 10).unwrap();
        assert_eq!(part.active, vec![3, 4, 5, 6]);
        assert_eq!(part.n_electrons_active, 4);
    }
}
