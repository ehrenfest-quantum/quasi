// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! SWORD document ingestion — Quantum Adapter (3a).
//!
//! Translates SWORD documents (emitted by Arn's `/analyze?output=sword`)
//! into [`EhrenfestProgram`]s that Afana can compile.
//!
//! A SWORD document contains:
//! - `partition.graph` — nodes with diagonal fields `h_i` and frequencies
//!   `omega`, edges with couplings `j_ij` and sinC² classifications
//! - `partition.volatile_islands` — subgraphs requiring quantum treatment
//! - `partition.stable_edges` — edges that can be treated classically
//! - `provenance` — extractor type, domain, class name
//! - `noise` — structural, environmental, exploitable noise estimates
//!
//! The adapter maps each volatile island to an Ising Hamiltonian:
//! - Diagonal terms: `h_i × Z_i`
//! - Coupling terms: `J_ij × Z_i Z_j`
//!
//! Stable nodes connected to volatile islands become mean-field boundary
//! terms: their expectation values are fixed at `⟨Z⟩ = +1` (ground state
//! assumption), contributing `J_ij × Z_i` shifts to the volatile Hamiltonian.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::cbor::{
    EhrenfestProgram, EvolutionTime, Hamiltonian, NoiseConstraint, Observable, PauliOp,
    PauliOpEntry, PauliTerm, SystemDef,
};

// ── Defaults ────────────────────────────────────────────────────────────────

/// Default Trotter evolution time (microseconds) when not specified by SWORD.
const DEFAULT_TOTAL_US: f64 = 1.0;
/// Default number of Trotter steps.
const DEFAULT_STEPS: u32 = 10;
/// Default T1 relaxation time (microseconds).
const DEFAULT_T1_US: f64 = 100.0;
/// Default T2 dephasing time (microseconds).
const DEFAULT_T2_US: f64 = 50.0;

// ── Public API ──────────────────────────────────────────────────────────────

/// Convert a SWORD JSON document into an Ehrenfest program.
///
/// The SWORD document must have `recommended_path == "quantum"`. The
/// function extracts volatile islands from the partition graph and builds
/// an Ising-style Hamiltonian for quantum compilation.
pub fn sword_to_ehrenfest(sword_json: &Value) -> Result<EhrenfestProgram> {
    // ── Validate recommended_path ───────────────────────────────────────
    let recommended_path = sword_json
        .get("recommended_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if recommended_path != "quantum" {
        bail!(
            "SWORD document has recommended_path=\"{}\", expected \"quantum\"",
            recommended_path
        );
    }

    // ── Extract partition ───────────────────────────────────────────────
    let partition = sword_json
        .get("partition")
        .context("SWORD document missing 'partition' field")?;

    let graph = partition
        .get("graph")
        .context("partition missing 'graph' field")?;

    // ── Parse nodes ────────────────────────────────────────────────────
    let nodes = graph
        .get("nodes")
        .and_then(|v| v.as_array())
        .context("graph missing 'nodes' array")?;

    let mut node_h: HashMap<u64, f64> = HashMap::new();
    for node in nodes {
        let id = node
            .get("id")
            .and_then(|v| v.as_u64())
            .context("node missing integer 'id'")?;
        let h_i = node
            .get("h_i")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        node_h.insert(id, h_i);
    }

    // ── Parse edges (full graph) ───────────────────────────────────────
    let edges = graph
        .get("edges")
        .and_then(|v| v.as_array())
        .context("graph missing 'edges' array")?;

    let mut edge_j: HashMap<(u64, u64), f64> = HashMap::new();
    for edge in edges {
        let i = edge
            .get("i")
            .and_then(|v| v.as_u64())
            .context("edge missing integer 'i'")?;
        let j = edge
            .get("j")
            .and_then(|v| v.as_u64())
            .context("edge missing integer 'j'")?;
        let j_ij = edge
            .get("j_ij")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        // Store with canonical ordering (min, max).
        let key = if i <= j { (i, j) } else { (j, i) };
        edge_j.insert(key, j_ij);
    }

    // ── Parse volatile islands ─────────────────────────────────────────
    let volatile_islands = partition
        .get("volatile_islands")
        .and_then(|v| v.as_array())
        .context("partition missing 'volatile_islands' array")?;

    if volatile_islands.is_empty() {
        bail!("SWORD document has no volatile islands — nothing to compile");
    }

    // ── Collect stable node IDs ────────────────────────────────────────
    let stable_edges = partition
        .get("stable_edges")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut all_volatile_node_ids: HashSet<u64> = HashSet::new();
    for island in volatile_islands {
        if let Some(nids) = island.get("node_ids").and_then(|v| v.as_array()) {
            for nid in nids {
                if let Some(id) = nid.as_u64() {
                    all_volatile_node_ids.insert(id);
                }
            }
        }
    }

    // ── Build merged Hamiltonian from all volatile islands ──────────────
    // Collect all volatile node IDs and map them to contiguous qubit indices.
    let mut sorted_volatile: Vec<u64> = all_volatile_node_ids.iter().copied().collect();
    sorted_volatile.sort_unstable();

    let n_qubits = sorted_volatile.len();
    if n_qubits == 0 {
        bail!("SWORD volatile islands contain no nodes");
    }

    let node_to_qubit: HashMap<u64, usize> = sorted_volatile
        .iter()
        .enumerate()
        .map(|(qi, &nid)| (nid, qi))
        .collect();

    let mut terms: Vec<PauliTerm> = Vec::new();
    let mut constant_offset: f64 = 0.0;

    // ── Diagonal terms: h_i × Z_i ──────────────────────────────────────
    for &nid in &sorted_volatile {
        let h_i = node_h.get(&nid).copied().unwrap_or(0.0);
        if h_i.abs() < 1e-15 {
            continue;
        }
        let qi = node_to_qubit[&nid];
        terms.push(PauliTerm {
            coefficient: h_i,
            paulis: vec![PauliOpEntry {
                qubit: qi,
                axis: PauliOp::Z,
            }],
        });
    }

    // ── Coupling terms: J_ij × Z_i Z_j (between volatile nodes) ────────
    for (&(ni, nj), &j_ij) in &edge_j {
        if j_ij.abs() < 1e-15 {
            continue;
        }
        let qi = match node_to_qubit.get(&ni) {
            Some(&q) => q,
            None => continue, // not a volatile node
        };
        let qj = match node_to_qubit.get(&nj) {
            Some(&q) => q,
            None => {
                // nj is a stable boundary node — mean-field contribution.
                // Assume ⟨Z_stable⟩ = +1, so J_ij × Z_i × 1 = J_ij × Z_i.
                terms.push(PauliTerm {
                    coefficient: j_ij,
                    paulis: vec![PauliOpEntry {
                        qubit: qi,
                        axis: PauliOp::Z,
                    }],
                });
                continue;
            }
        };
        terms.push(PauliTerm {
            coefficient: j_ij,
            paulis: vec![
                PauliOpEntry {
                    qubit: qi,
                    axis: PauliOp::Z,
                },
                PauliOpEntry {
                    qubit: qj,
                    axis: PauliOp::Z,
                },
            ],
        });
    }

    // Handle the symmetric case: if ni is stable but nj is volatile.
    for (&(ni, nj), &j_ij) in &edge_j {
        if j_ij.abs() < 1e-15 {
            continue;
        }
        if !node_to_qubit.contains_key(&ni) && node_to_qubit.contains_key(&nj) {
            // ni is stable, nj is volatile — mean-field shift on nj.
            let qj = node_to_qubit[&nj];
            terms.push(PauliTerm {
                coefficient: j_ij,
                paulis: vec![PauliOpEntry {
                    qubit: qj,
                    axis: PauliOp::Z,
                }],
            });
        }
    }

    // ── Stable-only edges contribute to constant offset ────────────────
    for se in &stable_edges {
        if let (Some(ni), Some(nj)) = (
            se.get("i").and_then(|v| v.as_u64()),
            se.get("j").and_then(|v| v.as_u64()),
        ) {
            if !node_to_qubit.contains_key(&ni) && !node_to_qubit.contains_key(&nj) {
                let j_ij_val = se.get("j_ij").and_then(|v| v.as_f64()).unwrap_or(0.0);
                // Both stable: ⟨Z_i⟩ = ⟨Z_j⟩ = +1, contributes J_ij to constant.
                constant_offset += j_ij_val;
            }
        }
    }

    // Ensure we have at least one Hamiltonian term.
    if terms.is_empty() {
        bail!("SWORD volatile islands produced no Hamiltonian terms (all coefficients zero)");
    }

    // ── Extract noise estimates for constraints ────────────────────────
    let noise_section = sword_json.get("noise");
    let structural_noise = noise_section
        .and_then(|n| n.get("structural"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let environmental_noise = noise_section
        .and_then(|n| n.get("environmental"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // Scale T2 down if SWORD reports high noise — rough heuristic.
    let noise_penalty = 1.0 - (structural_noise + environmental_noise).min(0.9);
    let effective_t2 = DEFAULT_T2_US * noise_penalty;

    // ── Build the EhrenfestProgram ─────────────────────────────────────
    let dt_us = DEFAULT_TOTAL_US / DEFAULT_STEPS as f64;

    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset,
        },
        evolution: EvolutionTime {
            total_us: DEFAULT_TOTAL_US,
            steps: DEFAULT_STEPS,
            dt_us,
        },
        observables: vec![Observable::E],
        noise: NoiseConstraint {
            t1_us: DEFAULT_T1_US,
            t2_us: effective_t2,
            gate_fidelity_min: Some(0.99),
            readout_fidelity_min: None,
        },
    };

    Ok(program)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_sword() -> Value {
        json!({
            "partition": {
                "graph": {
                    "nodes": [
                        {"id": 0, "h_i": 0.5, "omega": 1.0},
                        {"id": 1, "h_i": -0.3, "omega": 1.0},
                        {"id": 2, "h_i": 0.0, "omega": 1.0}
                    ],
                    "edges": [
                        {"i": 0, "j": 1, "j_ij": 0.7, "sin_c_half": 0.8, "classification": "volatile"},
                        {"i": 1, "j": 2, "j_ij": 0.2, "sin_c_half": 0.1, "classification": "stable"}
                    ]
                },
                "volatile_islands": [
                    {"node_ids": [0, 1], "edges": [{"i": 0, "j": 1}]}
                ],
                "stable_edges": [
                    {"i": 1, "j": 2, "j_ij": 0.2}
                ],
                "threshold": 0.5
            },
            "provenance": {
                "extractor": "ising",
                "domain": "optimization",
                "class_name": "MaxCut"
            },
            "recommended_path": "quantum",
            "noise": {
                "structural": 0.1,
                "environmental": 0.05,
                "exploitable": 0.02
            }
        })
    }

    #[test]
    fn converts_minimal_sword_to_ehrenfest() {
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();

        assert_eq!(program.version, 1);
        assert_eq!(program.system.n_qubits, 2); // nodes 0 and 1
        assert!(!program.hamiltonian.terms.is_empty());
    }

    #[test]
    fn diagonal_terms_map_to_single_z() {
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();

        // Should have Z_0 term with coefficient 0.5 and Z_1 with -0.3.
        let single_z_terms: Vec<_> = program
            .hamiltonian
            .terms
            .iter()
            .filter(|t| t.paulis.len() == 1 && t.paulis[0].axis == PauliOp::Z)
            .collect();

        // At least the h_i terms (may also include boundary mean-field terms).
        assert!(single_z_terms.len() >= 2);
    }

    #[test]
    fn coupling_terms_map_to_zz() {
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();

        let zz_terms: Vec<_> = program
            .hamiltonian
            .terms
            .iter()
            .filter(|t| {
                t.paulis.len() == 2
                    && t.paulis.iter().all(|p| p.axis == PauliOp::Z)
            })
            .collect();

        assert_eq!(zz_terms.len(), 1);
        assert!((zz_terms[0].coefficient - 0.7).abs() < 1e-10);
    }

    #[test]
    fn boundary_edge_becomes_mean_field_z() {
        // Edge (1,2) where node 2 is stable — should produce J*Z_1 term.
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();

        // Node 1 maps to qubit 1. The boundary edge j_ij=0.2 should
        // contribute a Z_1 term with coefficient 0.2.
        let boundary_terms: Vec<_> = program
            .hamiltonian
            .terms
            .iter()
            .filter(|t| {
                t.paulis.len() == 1
                    && t.paulis[0].axis == PauliOp::Z
                    && t.paulis[0].qubit == 1
                    && (t.coefficient - 0.2).abs() < 1e-10
            })
            .collect();

        assert!(!boundary_terms.is_empty(), "expected boundary mean-field Z term");
    }

    #[test]
    fn rejects_non_quantum_path() {
        let mut sword = minimal_sword();
        sword["recommended_path"] = json!("classical");
        let err = sword_to_ehrenfest(&sword).unwrap_err();
        assert!(err.to_string().contains("classical"));
    }

    #[test]
    fn rejects_missing_partition() {
        let sword = json!({"recommended_path": "quantum"});
        let err = sword_to_ehrenfest(&sword).unwrap_err();
        assert!(err.to_string().contains("partition"));
    }

    #[test]
    fn rejects_empty_volatile_islands() {
        let sword = json!({
            "partition": {
                "graph": {
                    "nodes": [{"id": 0, "h_i": 1.0, "omega": 1.0}],
                    "edges": []
                },
                "volatile_islands": [],
                "stable_edges": [],
                "threshold": 0.5
            },
            "recommended_path": "quantum",
            "noise": {"structural": 0.0, "environmental": 0.0, "exploitable": 0.0}
        });
        let err = sword_to_ehrenfest(&sword).unwrap_err();
        assert!(err.to_string().contains("no volatile islands"));
    }

    #[test]
    fn noise_penalty_reduces_t2() {
        let mut sword = minimal_sword();
        sword["noise"]["structural"] = json!(0.4);
        sword["noise"]["environmental"] = json!(0.3);
        let program = sword_to_ehrenfest(&sword).unwrap();
        // penalty = 1 - (0.4+0.3) = 0.3, effective_t2 = 50 * 0.3 = 15.0
        assert!((program.noise.t2_us - 15.0).abs() < 1e-10);
    }

    #[test]
    fn observable_is_energy() {
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();
        assert_eq!(program.observables.len(), 1);
        assert!(matches!(program.observables[0], Observable::E));
    }

    #[test]
    fn evolution_parameters_consistent() {
        let sword = minimal_sword();
        let program = sword_to_ehrenfest(&sword).unwrap();
        let expected_dt = program.evolution.total_us / program.evolution.steps as f64;
        assert!((program.evolution.dt_us - expected_dt).abs() < 1e-12);
    }
}
