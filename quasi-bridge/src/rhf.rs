// SPDX-License-Identifier: AGPL-3.0-or-later
//! Restricted Hartree-Fock solver.
//!
//! Implements the Roothaan-Hall SCF procedure (Szabo & Ostlund, Ch. 3):
//!   1. Orthogonaliser X = S^{-1/2}
//!   2. Core Hamiltonian H_core = T + V
//!   3. Initial guess: C from H_core eigenvectors
//!   4. Loop: D → F → F' = X^T F X → diag → C → D → check convergence
//!   5. Return MO coefficients, orbital energies, total energy

use crate::error::BridgeError;
use crate::integrals::MolecularIntegrals;
use nalgebra::{DMatrix, DVector, SymmetricEigen};

/// Result of an RHF calculation.
#[derive(Debug, Clone)]
pub struct RhfResult {
    /// Total RHF energy (electronic + nuclear repulsion) in Hartree.
    pub energy: f64,
    /// Electronic energy (without nuclear repulsion).
    pub electronic_energy: f64,
    /// MO coefficient matrix C (AO × MO). Column i is MO i.
    pub mo_coefficients: DMatrix<f64>,
    /// Orbital energies (eigenvalues of Fock matrix).
    pub orbital_energies: DVector<f64>,
    /// Number of occupied spatial orbitals.
    pub n_occupied: usize,
    /// Number of SCF iterations.
    pub iterations: usize,
}

/// Run Restricted Hartree-Fock.
///
/// `n_electrons` must be even (closed-shell).
pub fn rhf(ints: &MolecularIntegrals, n_electrons: usize) -> Result<RhfResult, BridgeError> {
    let n = ints.n_basis;
    let n_occ = n_electrons / 2;

    // Build nalgebra matrices from integrals
    let s = DMatrix::from_fn(n, n, |i, j| ints.overlap[i][j]);
    let h_core = DMatrix::from_fn(n, n, |i, j| ints.kinetic[i][j] + ints.nuclear[i][j]);

    // Orthogonaliser: X = S^{-1/2}
    let x = symmetric_orthogonaliser(&s);

    // Initial Fock = H_core → diag in orthogonal basis → initial C
    let f_prime = x.transpose() * &h_core * &x;
    let eigen = SymmetricEigen::new(f_prime);
    let sorted = sort_eigen(&eigen);
    let mut c = &x * &sorted.0;
    let mut epsilon = sorted.1;

    // SCF loop
    let max_iter = 200;
    let thresh = 1e-10;
    let mut e_old = 0.0;
    #[allow(unused_assignments)]
    let mut iterations = 0usize;
    let mut last_delta = f64::INFINITY;

    for iter in 0..max_iter {
        iterations = iter + 1;

        // Density matrix: D = 2 C_occ C_occ^T
        let c_occ = c.columns(0, n_occ);
        let d = 2.0 * &c_occ * c_occ.transpose();

        // Build Fock matrix: F_μν = H_core_μν + G_μν
        // G_μν = Σ_{λσ} D_{λσ} [(μν|λσ) - 0.5 (μλ|νσ)]
        let mut f = h_core.clone();
        for mu in 0..n {
            for nu in 0..n {
                let mut g = 0.0;
                for lam in 0..n {
                    for sig in 0..n {
                        let d_ls = d[(lam, sig)];
                        if d_ls.abs() < 1e-16 {
                            continue;
                        }
                        let j = ints.eri(mu, nu, lam, sig);
                        let k = ints.eri(mu, lam, nu, sig);
                        g += d_ls * (j - 0.5 * k);
                    }
                }
                f[(mu, nu)] += g;
            }
        }

        // Electronic energy: E_elec = 0.5 tr(D (H_core + F))
        let hf_sum = &h_core + &f;
        let e_elec = 0.5 * (&d * &hf_sum).trace();

        // Convergence check
        let delta_e = (e_elec - e_old).abs();
        last_delta = delta_e;
        e_old = e_elec;

        if delta_e < thresh && iter > 0 {
            return Ok(RhfResult {
                energy: e_elec + ints.nuclear_repulsion,
                electronic_energy: e_elec,
                mo_coefficients: c,
                orbital_energies: epsilon,
                n_occupied: n_occ,
                iterations,
            });
        }

        // Diagonalise F in orthogonal basis
        let f_prime = x.transpose() * &f * &x;
        let eigen = SymmetricEigen::new(f_prime);
        let sorted = sort_eigen(&eigen);
        c = &x * &sorted.0;
        epsilon = sorted.1;
    }

    Err(BridgeError::RhfNotConverged {
        iterations: max_iter,
        delta_e: last_delta,
    })
}

/// Compute S^{-1/2} via diagonalisation: S = U D U^T → S^{-1/2} = U D^{-1/2} U^T.
fn symmetric_orthogonaliser(s: &DMatrix<f64>) -> DMatrix<f64> {
    let eigen = SymmetricEigen::new(s.clone());
    let n = s.nrows();
    let mut d_inv_sqrt = DMatrix::zeros(n, n);
    for i in 0..n {
        let val = eigen.eigenvalues[i];
        if val > 1e-10 {
            d_inv_sqrt[(i, i)] = 1.0 / val.sqrt();
        }
    }
    &eigen.eigenvectors * d_inv_sqrt * eigen.eigenvectors.transpose()
}

/// Sort eigenvectors by eigenvalue (ascending).
fn sort_eigen(eigen: &SymmetricEigen<f64, nalgebra::Dyn>) -> (DMatrix<f64>, DVector<f64>) {
    let n = eigen.eigenvalues.len();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        eigen.eigenvalues[a]
            .partial_cmp(&eigen.eigenvalues[b])
            .unwrap()
    });

    let sorted_vals = DVector::from_fn(n, |i, _| eigen.eigenvalues[indices[i]]);
    let sorted_vecs = DMatrix::from_fn(n, n, |i, j| eigen.eigenvectors[(i, indices[j])]);
    (sorted_vecs, sorted_vals)
}

/// Transform one-electron integrals from AO to MO basis.
///
/// h_pq = Σ_{μν} C_{μp} h_{μν} C_{νq}
pub fn ao_to_mo_one(h_ao: &[Vec<f64>], c: &DMatrix<f64>) -> Vec<Vec<f64>> {
    let n = h_ao.len();
    let h_ao_mat = DMatrix::from_fn(n, n, |i, j| h_ao[i][j]);
    let h_mo = c.transpose() * h_ao_mat * c;
    let mut result = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            result[i][j] = h_mo[(i, j)];
        }
    }
    result
}

/// Transform two-electron integrals from AO to MO basis via 4-index transform.
///
/// (pq|rs) = Σ_{μνλσ} C_{μp} C_{νq} (μν|λσ) C_{λr} C_{σs}
pub fn ao_to_mo_two(ints: &MolecularIntegrals, c: &DMatrix<f64>) -> Vec<f64> {
    let n = ints.n_basis;

    // 4-step quarter-transform (O(n^5) each step, total O(n^5))
    // Step 1: (pν|λσ) = Σ_μ C_{μp} (μν|λσ)
    let mut tmp1 = vec![0.0; n * n * n * n];
    for p in 0..n {
        for nu in 0..n {
            for lam in 0..n {
                for sig in 0..n {
                    let mut val = 0.0;
                    for mu in 0..n {
                        val += c[(mu, p)] * ints.eri(mu, nu, lam, sig);
                    }
                    tmp1[p * n * n * n + nu * n * n + lam * n + sig] = val;
                }
            }
        }
    }

    // Step 2: (pq|λσ) = Σ_ν C_{νq} (pν|λσ)
    let mut tmp2 = vec![0.0; n * n * n * n];
    for p in 0..n {
        for q in 0..n {
            for lam in 0..n {
                for sig in 0..n {
                    let mut val = 0.0;
                    for nu in 0..n {
                        val += c[(nu, q)] * tmp1[p * n * n * n + nu * n * n + lam * n + sig];
                    }
                    tmp2[p * n * n * n + q * n * n + lam * n + sig] = val;
                }
            }
        }
    }

    // Step 3: (pq|rσ) = Σ_λ C_{λr} (pq|λσ)
    let mut tmp3 = vec![0.0; n * n * n * n];
    for p in 0..n {
        for q in 0..n {
            for r in 0..n {
                for sig in 0..n {
                    let mut val = 0.0;
                    for lam in 0..n {
                        val += c[(lam, r)] * tmp2[p * n * n * n + q * n * n + lam * n + sig];
                    }
                    tmp3[p * n * n * n + q * n * n + r * n + sig] = val;
                }
            }
        }
    }

    // Step 4: (pq|rs) = Σ_σ C_{σs} (pq|rσ)
    let mut mo_eri = vec![0.0; n * n * n * n];
    for p in 0..n {
        for q in 0..n {
            for r in 0..n {
                for s in 0..n {
                    let mut val = 0.0;
                    for sig in 0..n {
                        val += c[(sig, s)] * tmp3[p * n * n * n + q * n * n + r * n + sig];
                    }
                    mo_eri[p * n * n * n + q * n * n + r * n + s] = val;
                }
            }
        }
    }
    mo_eri
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis;
    use crate::integrals::compute_integrals;
    use crate::molecule;

    #[test]
    fn h2_rhf_energy() {
        let atoms = molecule::from_smiles("[H][H]").unwrap();
        let bf = basis::build_basis(&atoms, "sto-3g").unwrap();
        let ints = compute_integrals(&atoms, &bf);
        let result = rhf(&ints, 2).unwrap();

        // RHF/STO-3G energy for H2 at R=1.4 bohr ≈ -1.1175 Ha
        assert!(
            (result.energy - (-1.1175)).abs() < 0.005,
            "H2 RHF energy = {:.6}, expected ~-1.1175",
            result.energy
        );
        assert!(result.iterations < 50, "took {} iterations", result.iterations);
    }
}
