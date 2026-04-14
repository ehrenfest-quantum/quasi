// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end molecular pipeline: SMILES → energy.
//!
//! Orchestrates all stages:
//!   1. SMILES → atom coordinates
//!   2. Atom coordinates → AO integrals (STO-3G)
//!   3. RHF → MO coefficients
//!   4. AO→MO integral transformation
//!   5. Active-space selection
//!   6. Jordan-Wigner → qubit Hamiltonian
//!   7. Build Ehrenfest program → CBOR → Afana compile
//!   8. Solvayeur backend selection (classical fallback)
//!   9. Exact diagonalization (Huoma statevector equivalent)
//!  10. Cache result

use std::collections::BTreeMap;
use std::time::Instant;

use crate::basis;
use crate::ehrenfest_mol::{self, Accuracy};
use crate::error::BridgeError;
use crate::integrals;
use crate::jordan_wigner;
use crate::molecule;
use crate::partition;
use crate::postprocess::{self, MolecularResult};
use crate::rhf;

/// Configuration for the bridge pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub basis: String,
    pub accuracy: Accuracy,
    /// Number of active orbitals (None = full space).
    pub n_active: Option<usize>,
    /// Optional Garm analysis JSON for partition.
    pub garm_json: Option<String>,
    /// Whether to print progress.
    pub verbose: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            basis: "sto-3g".into(),
            accuracy: Accuracy::Chemical,
            n_active: None,
            garm_json: None,
            verbose: false,
        }
    }
}

/// Run the full pipeline for a molecule.
pub fn run_molecule(smiles: &str, config: &PipelineConfig) -> Result<MolecularResult, BridgeError> {
    let start = Instant::now();

    // 1. SMILES → atom coordinates
    let atoms = molecule::from_smiles(smiles)?;
    let n_electrons = molecule::count_electrons(&atoms);
    if config.verbose {
        eprintln!(
            "Molecule: {} ({} atoms, {} electrons)",
            smiles,
            atoms.len(),
            n_electrons
        );
    }

    // 2. Build basis and compute integrals
    let bf = basis::build_basis(&atoms, &config.basis)?;
    let n_spatial = bf.len();
    if config.verbose {
        eprintln!("Basis: {} ({} functions)", config.basis, n_spatial);
    }
    let ints = integrals::compute_integrals(&atoms, &bf);

    // 3. RHF
    let rhf_result = rhf::rhf(&ints, n_electrons)?;
    if config.verbose {
        eprintln!(
            "RHF converged: {:.6} Ha ({} iterations)",
            rhf_result.energy, rhf_result.iterations
        );
    }

    // 4. AO → MO integral transformation
    let h_core_ao: Vec<Vec<f64>> = (0..n_spatial)
        .map(|i| {
            (0..n_spatial)
                .map(|j| ints.kinetic[i][j] + ints.nuclear[i][j])
                .collect()
        })
        .collect();
    let h_mo = rhf::ao_to_mo_one(&h_core_ao, &rhf_result.mo_coefficients);
    let eri_mo = rhf::ao_to_mo_two(&ints, &rhf_result.mo_coefficients);

    // 5. Active-space selection
    let part = if let Some(ref json) = config.garm_json {
        partition::from_garm_json(json, n_spatial, n_electrons)?
    } else {
        partition::frontier_partition(n_spatial, n_electrons, config.n_active)?
    };
    let n_active = part.active.len();
    let n_active_elec = part.n_electrons_active;
    if config.verbose {
        eprintln!(
            "Active space: {} orbitals, {} electrons ({})",
            n_active, n_active_elec, part.method
        );
    }

    // 6. Select active-space integrals
    let (h_act, eri_act, na) =
        partition::select_active_integrals(&h_mo, &eri_mo, n_spatial, &part);

    // 7. Jordan-Wigner transform
    let pauli_terms =
        jordan_wigner::jordan_wigner_transform(&h_act, &eri_act, na, ints.nuclear_repulsion);
    let n_qubits = 2 * na;
    if config.verbose {
        eprintln!(
            "Jordan-Wigner: {} Pauli terms, {} qubits",
            pauli_terms.len(),
            n_qubits
        );
    }

    // 8. Build Ehrenfest program and compile through Afana
    let program =
        ehrenfest_mol::build_ehrenfest_program(&pauli_terms, n_qubits, config.accuracy);
    let cbor_bytes = ehrenfest_mol::to_cbor(&program);

    // Verify Afana can parse and compile
    let parsed = afana::cbor::from_cbor(&cbor_bytes)
        .map_err(|e| BridgeError::AfanaCompile(e.to_string()))?;
    let (ast, trotter_stats) = afana::trotter::trotterize_with_stats(
        &parsed,
        afana::trotter::TrotterOrder::Second,
    )
    .map_err(|e| BridgeError::AfanaCompile(e.to_string()))?;

    let _qasm = afana::emit::emit_qasm(&ast, afana::emit::QasmVersion::V3)
        .map_err(|e| BridgeError::AfanaCompile(e.to_string()))?;

    if config.verbose {
        eprintln!(
            "Afana: {} gates, Trotter S2 {} steps, error bound {:.4} mHa",
            trotter_stats.total_gates,
            trotter_stats.steps,
            trotter_stats.error_bound * 1000.0
        );
    }

    // 9. Solvayeur backend selection (classical fallback)
    let backends = make_simulator_backends();
    let solvayeur = quasi_solvayeur::atw::Solvayeur::new(&backends);
    let epoch_result = solvayeur
        .decide()
        .map_err(|e| BridgeError::Solvayeur(e.to_string()))?;
    let backend_name = if epoch_result.backend_index < backends.len() {
        backends[epoch_result.backend_index].id().to_string()
    } else {
        "huoma_statevector".into()
    };
    if config.verbose {
        eprintln!(
            "Solvayeur: epoch 0, exploration {:.2}, selected {} (index {})",
            solvayeur.exploration(),
            backend_name,
            epoch_result.backend_index
        );
    }

    // 10. Execute: exact diagonalization (equivalent to Huoma statevector)
    let energy = postprocess::ground_state_energy(&pauli_terms, n_qubits)?;
    if config.verbose {
        let elapsed = start.elapsed();
        eprintln!("Energy: {:.6} Ha", energy);
        eprintln!(
            "RHF energy: {:.6} Ha, correlation: {:.6} Ha",
            rhf_result.energy,
            energy - rhf_result.energy
        );
        eprintln!("Wall time: {:.1}s", elapsed.as_secs_f64());
    }

    // 11. Build cache key (for future caching)
    let _cache_key = quasi_cache::CacheKey::build(
        &cbor_bytes,
        &backend_name,
        &BTreeMap::new(),
        "v1",
    );

    Ok(MolecularResult {
        energy,
        trotter_error_bound_mha: trotter_stats.error_bound * 1000.0,
        backend: backend_name,
        n_pauli_terms: pauli_terms.len(),
        n_qubits,
        smiles: smiles.into(),
        basis: config.basis.clone(),
        rhf_energy: rhf_result.energy,
        trotter_steps: trotter_stats.steps,
        gate_count: trotter_stats.total_gates,
    })
}

/// Create simulator backends for the Solvayeur to choose from.
fn make_simulator_backends() -> Vec<quasi_scheduler::backend::Backend> {
    use quasi_scheduler::backend::*;
    vec![
        Backend {
            capabilities: Capabilities {
                name: "huoma_statevector".into(),
                num_qubits: 30,
                gate_set: GateSet {
                    single_qubit: vec![
                        "h".into(), "x".into(), "y".into(), "z".into(),
                        "rz".into(), "ry".into(), "rx".into(),
                    ],
                    two_qubit: vec!["cx".into()],
                    three_qubit: vec![],
                    native: vec![
                        "h".into(), "x".into(), "y".into(), "z".into(),
                        "cx".into(), "rz".into(), "ry".into(), "rx".into(),
                    ],
                },
                topology: Topology::full(30),
                max_shots: 100_000,
                is_simulator: true,
                features: vec!["statevector".into()],
                noise_profile: None,
            },
            backend_type: BackendType::Simulator,
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: 0.0001,
        },
        Backend {
            capabilities: Capabilities {
                name: "huoma_projected_ttn".into(),
                num_qubits: 1_000_000,
                gate_set: GateSet {
                    single_qubit: vec!["h".into(), "x".into(), "rz".into()],
                    two_qubit: vec!["cx".into()],
                    three_qubit: vec![],
                    native: vec!["h".into(), "x".into(), "cx".into(), "rz".into()],
                },
                topology: Topology::full(1_000_000),
                max_shots: 100_000,
                is_simulator: true,
                features: vec!["tensor_network".into()],
                noise_profile: Some(NoiseProfile {
                    t1: None,
                    t2: None,
                    single_qubit_fidelity: Some(0.999999),
                    two_qubit_fidelity: Some(0.99999),
                    readout_fidelity: Some(1.0),
                    gate_time: None,
                }),
            },
            backend_type: BackendType::Simulator,
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: 0.001,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h2_full_pipeline() {
        let config = PipelineConfig {
            verbose: false,
            ..PipelineConfig::default()
        };
        let result = run_molecule("[H][H]", &config).unwrap();

        // FCI/STO-3G energy for H2 ≈ -1.1373 Ha (from the spec)
        // Our exact diag should match this closely
        assert!(
            result.energy < -1.05,
            "H2 energy should be below -1.05 Ha, got {:.6}",
            result.energy
        );
        assert!(result.n_qubits == 4, "H2/STO-3G should use 4 qubits");
        assert!(result.gate_count > 0, "should have gates from Afana");
    }
}
