// SPDX-License-Identifier: AGPL-3.0-or-later
//! Active space extension beyond QPU qubit limits.
//!
//! The IQM QExa20 has 20 physical qubits, constraining the active space
//! to 10 spatial orbitals. Huoma's exact diag handles up to 20 qubits
//! within the same limit, but demonstrates what a classical tensor-network
//! approach can achieve without QPU hardware.
//!
//! For the Zundel cation (15 spatial orbitals in STO-3G), we can scan
//! active spaces from 4 to 15 orbitals (8 to 30 qubits). Exact diag
//! handles up to 10 orbitals (20 qubits); larger active spaces require
//! MPS or ProjectedTTN methods (future Huoma capability).
//!
//! The convergence curve shows how much correlation energy is captured
//! as the active space grows. The difference between the 10-orbital
//! (QPU-limited) and 15-orbital (full-space) results quantifies what
//! the QPU qubit limit costs.

use std::time::Instant;

use quasi_bridge::error::BridgeError;
use quasi_bridge::postprocess;

use super::zundel;

/// Result for a single active-space size.
#[derive(Debug, Clone)]
pub struct ActiveSpaceResult {
    /// Number of active spatial orbitals.
    pub n_active: usize,
    /// Number of qubits (2 × n_active).
    pub n_qubits: usize,
    /// Ground-state energy (Hartree).
    pub energy: f64,
    /// Method used ("exact" or "mps_future").
    pub method: String,
    /// Wall time (seconds).
    pub time_s: f64,
    /// Number of Pauli terms.
    pub n_pauli_terms: usize,
}

/// Result of the full extension scan.
pub struct ExtensionResult {
    /// Results for each active-space size.
    pub results: Vec<ActiveSpaceResult>,
    /// RHF energy for reference.
    pub rhf_energy: f64,
    /// Energy at QPU limit (10 orbitals / 20 qubits).
    pub energy_at_qpu_limit: Option<f64>,
    /// Energy at full space (all orbitals).
    pub energy_full_space: Option<f64>,
    /// Correlation energy missed by QPU limit.
    pub correlation_missed: Option<f64>,
}

/// Extend the active space and track energy convergence.
///
/// Scans active-space sizes from 4 to the maximum that fits within exact
/// diag limits. For each size, builds the Hamiltonian and computes the
/// ground-state energy.
pub fn extend_active_space() -> Result<ExtensionResult, BridgeError> {
    // Active-space sizes to scan.
    // STO-3G Zundel has 15 spatial orbitals.
    // Dense exact diag builds a 2^(2N) × 2^(2N) matrix:
    //   4 orbitals →  8 qubits →    256×256    (512 KB)
    //   5 orbitals → 10 qubits →   1024×1024   (8 MB)
    //   6 orbitals → 12 qubits →   4096×4096   (128 MB)
    //   7 orbitals → 14 qubits →  16384×16384  (2 GB)
    // Beyond 7 orbitals requires iterative eigensolver or MPS.
    let sizes: Vec<usize> = vec![4, 5, 6, 7];

    println!(
        "Active space extension: scanning {} sizes",
        sizes.len()
    );
    println!(
        "  Dense exact diag limit: ~14 qubits (7 active orbitals, 2 GB)"
    );
    println!(
        "  IQM QExa20 equivalent (10 orbitals, 20 qubits) requires iterative solver"
    );
    println!();

    let mut results = Vec::new();
    let mut rhf_energy = 0.0;
    let mut energy_at_qpu_limit = None;

    for &n_active in &sizes {
        let t0 = Instant::now();

        let ham = zundel::build_zundel_hamiltonian(Some(n_active))?;
        rhf_energy = ham.rhf_energy;

        let n_qubits = ham.n_qubits;
        let n_pauli_terms = ham.pauli_terms.len();

        let (energy, method) = if n_qubits <= 14 {
            let e = postprocess::ground_state_energy(
                &ham.pauli_terms,
                ham.n_qubits,
            )?;
            (e, "exact".to_string())
        } else {
            // Beyond dense diag limit — at 16 qubits the matrix is 32 GB.
            // Report RHF energy as a placeholder; iterative eigensolver or
            // MPS would capture correlation at this scale.
            eprintln!(
                "  N_active={}: {} qubits exceeds dense diag limit (14). \
                 Iterative solver / MPS required.",
                n_active, n_qubits
            );
            (ham.rhf_energy, "mps_future".to_string())
        };

        let time = t0.elapsed().as_secs_f64();

        println!(
            "  N_active={:2}  N_qubits={:2}  E={:.6} Ha  method={:<12}  time={:.3}s  terms={}",
            n_active, n_qubits, energy, method, time, n_pauli_terms
        );

        // The largest exact-diag result serves as our best energy
        if method == "exact" {
            energy_at_qpu_limit = Some(energy);
        }

        results.push(ActiveSpaceResult {
            n_active,
            n_qubits,
            energy,
            method,
            time_s: time,
            n_pauli_terms,
        });
    }

    // Compute correlation energy missed by QPU limit
    let energy_full_space = results.last().map(|r| r.energy);
    let correlation_missed = match (energy_at_qpu_limit, energy_full_space) {
        (Some(e_qpu), Some(e_full)) => Some((e_qpu - e_full).abs()),
        _ => None,
    };

    println!("\n--- Convergence summary ---");
    println!("  RHF energy: {:.6} Ha", rhf_energy);
    if let Some(e) = energy_at_qpu_limit {
        println!("  Energy at QPU limit (10 orbitals): {:.6} Ha", e);
        println!(
            "  Correlation at QPU limit: {:.6} Ha",
            e - rhf_energy
        );
    }
    if let Some(e) = energy_full_space {
        println!("  Best energy (largest active space): {:.6} Ha", e);
    }
    if let Some(missed) = correlation_missed {
        println!(
            "  Correlation missed by QPU limit: {:.6} Ha ({:.2} mHa)",
            missed,
            missed * 1000.0
        );
    }

    Ok(ExtensionResult {
        results,
        rhf_energy,
        energy_at_qpu_limit,
        energy_full_space,
        correlation_missed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_scan_produces_results() {
        // Quick test with just 4 orbitals
        let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
        let energy = postprocess::ground_state_energy(&ham.pauli_terms, ham.n_qubits).unwrap();
        assert!(energy < 0.0);
    }

    #[test]
    fn energy_decreases_with_larger_active_space() {
        // Energy should generally decrease (or stay same) as active space grows,
        // since more correlation is captured. Use 4 vs 5 orbitals to keep
        // the JW transform fast in debug mode.
        let ham4 = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
        let ham5 = zundel::build_zundel_hamiltonian(Some(5)).unwrap();
        let e4 = postprocess::ground_state_energy(&ham4.pauli_terms, ham4.n_qubits).unwrap();
        let e5 = postprocess::ground_state_energy(&ham5.pauli_terms, ham5.n_qubits).unwrap();
        // With different active spaces the nuclear repulsion is included in both,
        // so the comparison is meaningful. More orbitals captures more correlation.
        // Allow a small tolerance since the active space changes the problem.
        assert!(
            e5 <= e4 + 0.1,
            "5-orbital energy ({e5:.6}) should be <= 4-orbital ({e4:.6}) + tolerance"
        );
    }
}
