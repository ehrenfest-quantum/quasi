// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commensurability analysis of the Zundel Hamiltonian.
//!
//! Applies sin(C/2) analysis to the qubit Hamiltonian's coupling structure.
//! The coupling graph is extracted from the Pauli terms:
//!   - ZZ, XX, YY, XY, ... terms with non-trivial operators on two qubits
//!     define edges with weight = |coefficient|
//!   - Z terms define single-qubit fields (nodes)
//!
//! sin(C/2) of the coupling strength C between qubit pairs determines
//! whether they are:
//!   - Commensurate (high sin(C/2) → strong entanglement, need high bond
//!     dimension χ for tensor-network simulation)
//!   - Incommensurate (low sin(C/2) → weak entanglement, can collapse to χ=1)
//!
//! If sin(C/2) identifies the same active orbitals that Coveney's group
//! selected by chemical intuition (the proton-transfer σ/σ* and O lone
//! pairs), that validates QUASI's automated partitioning.

use std::collections::HashMap;

use quasi_bridge::error::BridgeError;

use super::zundel;

/// A qubit-pair coupling entry.
#[derive(Debug, Clone)]
pub struct CouplingEdge {
    pub qubit_a: usize,
    pub qubit_b: usize,
    /// Sum of |coefficient| for all Pauli terms coupling this pair.
    pub coupling_strength: f64,
    /// sin(C/2) where C = coupling_strength.
    pub sin_c2: f64,
}

/// Single-qubit field entry.
#[derive(Debug, Clone)]
pub struct FieldNode {
    pub qubit: usize,
    /// Sum of |coefficient| for all single-qubit Z terms on this qubit.
    pub field_strength: f64,
}

/// Commensurability partition.
#[derive(Debug, Clone)]
pub struct CommensurabilityPartition {
    /// Qubit pairs with sin(C/2) above threshold (strongly entangled).
    pub commensurate: Vec<CouplingEdge>,
    /// Qubit pairs with sin(C/2) below threshold (weakly entangled).
    pub incommensurate: Vec<CouplingEdge>,
    /// Threshold used for partitioning.
    pub threshold: f64,
}

/// Result of the commensurability analysis.
pub struct AnalysisResult {
    /// All coupling edges.
    pub edges: Vec<CouplingEdge>,
    /// All field nodes.
    pub fields: Vec<FieldNode>,
    /// Partition into commensurate/incommensurate.
    pub partition: CommensurabilityPartition,
    /// Number of qubits.
    pub n_qubits: usize,
    /// Active orbital indices (from sin(C/2) — qubits with high coupling).
    pub sinc2_active_qubits: Vec<usize>,
    /// Active orbital indices (from frontier partition — manual/heuristic).
    pub frontier_active_orbitals: Vec<usize>,
    /// Compression ratio: total pairs / commensurate pairs.
    pub compression_ratio: f64,
}

/// Extract the coupling graph from a Pauli Hamiltonian.
///
/// For each Pauli term, identify which qubits have non-identity operators.
/// If exactly two qubits are non-identity, it's an edge. If one, it's a field.
pub fn extract_coupling_graph(
    pauli_terms: &[(f64, String)],
    n_qubits: usize,
) -> (Vec<CouplingEdge>, Vec<FieldNode>) {
    let mut edge_map: HashMap<(usize, usize), f64> = HashMap::new();
    let mut field_map: HashMap<usize, f64> = HashMap::new();

    for (coeff, pauli_str) in pauli_terms {
        let non_identity: Vec<usize> = pauli_str
            .chars()
            .enumerate()
            .filter(|(_, ch)| *ch != 'I')
            .map(|(i, _)| i)
            .collect();

        match non_identity.len() {
            1 => {
                *field_map.entry(non_identity[0]).or_insert(0.0) += coeff.abs();
            }
            2 => {
                let key = (non_identity[0].min(non_identity[1]),
                           non_identity[0].max(non_identity[1]));
                *edge_map.entry(key).or_insert(0.0) += coeff.abs();
            }
            n if n > 2 => {
                // Multi-body terms: add contributions to all pairs
                for i in 0..non_identity.len() {
                    for j in (i + 1)..non_identity.len() {
                        let key = (non_identity[i], non_identity[j]);
                        *edge_map.entry(key).or_insert(0.0) += coeff.abs() / n as f64;
                    }
                }
            }
            _ => {} // pure identity — skip
        }
    }

    let edges: Vec<CouplingEdge> = edge_map
        .into_iter()
        .map(|((a, b), c)| CouplingEdge {
            qubit_a: a,
            qubit_b: b,
            coupling_strength: c,
            sin_c2: (c / 2.0).sin(),
        })
        .collect();

    let fields: Vec<FieldNode> = field_map
        .into_iter()
        .map(|(q, f)| FieldNode {
            qubit: q,
            field_strength: f,
        })
        .collect();

    let _ = n_qubits; // used for context, not filtering
    (edges, fields)
}

/// Partition coupling edges by sin(C/2) threshold.
pub fn partition_by_sinc2(
    edges: &[CouplingEdge],
    threshold: f64,
) -> CommensurabilityPartition {
    let mut commensurate = Vec::new();
    let mut incommensurate = Vec::new();

    for edge in edges {
        if edge.sin_c2 >= threshold {
            commensurate.push(edge.clone());
        } else {
            incommensurate.push(edge.clone());
        }
    }

    CommensurabilityPartition {
        commensurate,
        incommensurate,
        threshold,
    }
}

/// Identify active qubits from the coupling graph: qubits that participate
/// in at least one commensurate (high sin(C/2)) edge.
fn active_qubits_from_partition(partition: &CommensurabilityPartition) -> Vec<usize> {
    let mut qubits: Vec<usize> = partition
        .commensurate
        .iter()
        .flat_map(|e| vec![e.qubit_a, e.qubit_b])
        .collect();
    qubits.sort_unstable();
    qubits.dedup();
    qubits
}

/// Compute Jaccard similarity between two sets.
fn jaccard_similarity(a: &[usize], b: &[usize]) -> f64 {
    let set_a: std::collections::HashSet<usize> = a.iter().copied().collect();
    let set_b: std::collections::HashSet<usize> = b.iter().copied().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Run the full commensurability analysis on the Zundel cation.
pub fn analyse_zundel() -> Result<AnalysisResult, BridgeError> {
    // Build Hamiltonian with 10 active orbitals to match IQM setup
    let ham = zundel::build_zundel_hamiltonian(Some(10))?;

    println!(
        "Zundel H5O2+ commensurability analysis ({} qubits, {} Pauli terms)",
        ham.n_qubits,
        ham.pauli_terms.len()
    );

    // Extract coupling graph
    let (mut edges, mut fields) = extract_coupling_graph(&ham.pauli_terms, ham.n_qubits);
    edges.sort_by(|a, b| b.sin_c2.partial_cmp(&a.sin_c2).unwrap());
    fields.sort_by(|a, b| b.field_strength.partial_cmp(&a.field_strength).unwrap());

    // Partition with threshold 0.3
    let threshold = 0.3;
    let partition = partition_by_sinc2(&edges, threshold);

    let total_pairs = edges.len();
    let n_commensurate = partition.commensurate.len();
    let n_incommensurate = partition.incommensurate.len();
    let compression_ratio = if n_commensurate > 0 {
        total_pairs as f64 / n_commensurate as f64
    } else {
        f64::INFINITY
    };

    // Active qubits from sin(C/2) analysis
    let sinc2_active = active_qubits_from_partition(&partition);

    // The frontier partition's active orbitals (mapped to qubit indices:
    // orbital i → qubits 2i and 2i+1 in the active-space-local basis)
    let frontier_active: Vec<usize> = (0..ham.n_active)
        .flat_map(|i| vec![2 * i, 2 * i + 1])
        .collect();

    // Jaccard similarity between sin(C/2) selection and frontier selection
    let jaccard = jaccard_similarity(&sinc2_active, &frontier_active);

    // Print results
    println!("\nCoupling graph:");
    println!("  Total qubit pairs: {}", total_pairs);
    println!("  Single-qubit fields: {}", fields.len());

    println!("\nsin(C/2) partition (threshold = {:.1}):", threshold);
    println!("  Commensurate (active): {}", n_commensurate);
    println!("  Incommensurate (environment): {}", n_incommensurate);
    println!("  Compression ratio: {:.1}x", compression_ratio);

    println!("\nTop 10 coupling edges by sin(C/2):");
    for edge in edges.iter().take(10) {
        println!(
            "  q{:2}-q{:2}  C={:.4}  sin(C/2)={:.4}",
            edge.qubit_a, edge.qubit_b, edge.coupling_strength, edge.sin_c2
        );
    }

    println!("\nActive-space comparison:");
    println!("  Qubits flagged by sin(C/2): {:?}", sinc2_active);
    println!("  Qubits from frontier partition: {:?}", frontier_active);
    println!("  Jaccard similarity: {:.3}", jaccard);

    if jaccard >= 0.8 {
        println!(
            "  PASS: sin(C/2) identifies the same active region as \
             frontier partition (Jaccard >= 0.8)"
        );
    } else {
        println!(
            "  NOTE: sin(C/2) selection differs from frontier partition \
             (Jaccard {:.3} < 0.8). This may indicate the entanglement \
             structure differs from the energy-based partition.",
            jaccard
        );
    }

    Ok(AnalysisResult {
        edges,
        fields,
        partition,
        n_qubits: ham.n_qubits,
        sinc2_active_qubits: sinc2_active,
        frontier_active_orbitals: ham.active_orbitals,
        compression_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_coupling_from_simple_hamiltonian() {
        let terms = vec![
            (0.5, "ZI".to_string()),
            (-0.3, "IZ".to_string()),
            (0.1, "XX".to_string()),
            (0.1, "ZZ".to_string()),
        ];
        let (edges, fields) = extract_coupling_graph(&terms, 2);
        assert_eq!(edges.len(), 1, "XX and ZZ both couple qubits 0-1");
        assert_eq!(fields.len(), 2);
        // Coupling strength = |0.1| + |0.1| = 0.2
        assert!((edges[0].coupling_strength - 0.2).abs() < 1e-10);
    }

    #[test]
    fn partition_separates_by_threshold() {
        let edges = vec![
            CouplingEdge { qubit_a: 0, qubit_b: 1, coupling_strength: 1.0, sin_c2: 0.479 },
            CouplingEdge { qubit_a: 0, qubit_b: 2, coupling_strength: 0.01, sin_c2: 0.005 },
        ];
        let part = partition_by_sinc2(&edges, 0.3);
        assert_eq!(part.commensurate.len(), 1);
        assert_eq!(part.incommensurate.len(), 1);
    }

    #[test]
    fn jaccard_identical_sets() {
        assert!((jaccard_similarity(&[1, 2, 3], &[1, 2, 3]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        assert!((jaccard_similarity(&[1, 2], &[3, 4]) - 0.0).abs() < 1e-10);
    }
}
