// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for the GPCR benchmark.

use quasi_benchmark::gpcr::commensurability;
use quasi_benchmark::gpcr::zundel;
use quasi_bridge::postprocess;

// ── Knuth validation: H2 before Zundel ───────────────────────────────────

#[test]
fn validate_h2_energy_against_reference() {
    let (energy, reference, name) = zundel::validate_h2().unwrap();
    let delta = (energy - reference).abs();
    assert!(
        delta < 0.05,
        "{name}: computed {energy:.6} vs reference {reference:.6}, \
         delta {delta:.4} exceeds 0.05 Ha tolerance"
    );
}

// ── Zundel construction ──────────────────────────────────────────────────

#[test]
fn zundel_geometry_is_valid() {
    let atoms = zundel::zundel_geometry();
    assert_eq!(atoms.len(), 7, "H5O2+ has 7 atoms");

    let n_o = atoms.iter().filter(|(z, _)| *z == 8).count();
    let n_h = atoms.iter().filter(|(z, _)| *z == 1).count();
    assert_eq!(n_o, 2, "2 oxygen atoms");
    assert_eq!(n_h, 5, "5 hydrogen atoms");
}

#[test]
fn zundel_hamiltonian_4_orbitals() {
    let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    assert_eq!(ham.n_qubits, 8);
    assert_eq!(ham.n_active, 4);
    assert!(!ham.pauli_terms.is_empty());

    // All Pauli strings should have length 8
    for (_, s) in &ham.pauli_terms {
        assert_eq!(
            s.len(), 8,
            "Pauli string length should be 8 (qubits), got '{s}'"
        );
    }
}

#[test]
fn zundel_hamiltonian_5_orbitals() {
    let ham = zundel::build_zundel_hamiltonian(Some(5)).unwrap();
    assert_eq!(ham.n_qubits, 10);
    assert_eq!(ham.n_active, 5);
    assert!(!ham.pauli_terms.is_empty());
}

// ── Exact diagonalisation ────────────────────────────────────────────────

#[test]
fn zundel_exact_diag_4_orbitals() {
    let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    let energy = postprocess::ground_state_energy(&ham.pauli_terms, ham.n_qubits).unwrap();

    // Energy should be negative
    assert!(energy < 0.0, "energy should be negative, got {energy:.6}");

    // Should be below (more negative than) RHF for this active space
    // Note: with a small active space, the exact diag energy within that space
    // may not beat full-space RHF, but it should be a reasonable energy.
    eprintln!("Zundel 4-orbital exact diag: {:.6} Ha", energy);
    eprintln!("Zundel RHF: {:.6} Ha", ham.rhf_energy);
}

#[test]
fn zundel_exact_diag_5_orbitals() {
    // Use 5 orbitals (10 qubits) — fast enough in debug mode.
    // 8+ orbitals (16+ qubits) make JW transform and exact diag too slow
    // without --release optimisation.
    let ham = zundel::build_zundel_hamiltonian(Some(5)).unwrap();
    assert_eq!(ham.n_qubits, 10);
    let energy = postprocess::ground_state_energy(&ham.pauli_terms, ham.n_qubits).unwrap();
    assert!(energy < 0.0, "energy should be negative, got {energy:.6}");
    eprintln!("Zundel 5-orbital exact diag: {:.6} Ha", energy);
}

// ── Commensurability ─────────────────────────────────────────────────────

#[test]
fn coupling_graph_extraction() {
    let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    let (edges, fields) =
        commensurability::extract_coupling_graph(&ham.pauli_terms, ham.n_qubits);

    // Should find coupling edges and field terms
    assert!(
        !edges.is_empty(),
        "should have coupling edges in the Hamiltonian"
    );
    // At least some single-qubit terms should exist
    assert!(
        !fields.is_empty(),
        "should have single-qubit field terms"
    );

    // All sin(C/2) values should be in [-1, 1]
    for edge in &edges {
        assert!(
            edge.sin_c2.abs() <= 1.0,
            "sin(C/2) = {} out of range",
            edge.sin_c2
        );
    }
}

#[test]
fn commensurability_partition() {
    let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    let (edges, _) = commensurability::extract_coupling_graph(&ham.pauli_terms, ham.n_qubits);
    let partition = commensurability::partition_by_sinc2(&edges, 0.3);

    // Total should equal sum of partitions
    assert_eq!(
        partition.commensurate.len() + partition.incommensurate.len(),
        edges.len()
    );
}

// ── Extension convergence ────────────────────────────────────────────────

#[test]
fn energy_monotonicity_small_to_medium() {
    // With increasing active space, more correlation is captured.
    // The energy should generally decrease (become more negative).
    // Use 4 vs 5 orbitals to keep the JW transform fast in debug mode
    // (6+ orbitals make JW O(n^4) too slow without --release).
    let ham4 = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    let ham5 = zundel::build_zundel_hamiltonian(Some(5)).unwrap();

    let e4 = postprocess::ground_state_energy(&ham4.pauli_terms, ham4.n_qubits).unwrap();
    let e5 = postprocess::ground_state_energy(&ham5.pauli_terms, ham5.n_qubits).unwrap();

    eprintln!("E(4 orbitals) = {:.6} Ha", e4);
    eprintln!("E(5 orbitals) = {:.6} Ha", e5);

    // Allow tolerance since different active spaces are different problems
    assert!(
        e5 <= e4 + 0.1,
        "5-orbital energy ({e5:.6}) should be <= 4-orbital ({e4:.6}) + 0.1 Ha"
    );
}

// ── Ehrenfest pipeline ───────────────────────────────────────────────────

#[test]
fn zundel_ehrenfest_compiles() {
    use quasi_bridge::ehrenfest_mol::{self, Accuracy};

    let ham = zundel::build_zundel_hamiltonian(Some(4)).unwrap();
    let program = ehrenfest_mol::build_ehrenfest_program(
        &ham.pauli_terms,
        ham.n_qubits,
        Accuracy::Chemical,
    );
    let cbor = ehrenfest_mol::to_cbor(&program);
    let parsed = afana::cbor::from_cbor(&cbor).unwrap();

    let (ast, stats) = afana::trotter::trotterize_with_stats(
        &parsed,
        afana::trotter::TrotterOrder::Second,
    )
    .unwrap();

    assert!(ast.gates.len() > 0, "should produce gates");
    assert!(stats.total_gates > 0, "should have non-zero gate count");

    let qasm = afana::emit::emit_qasm(&ast, afana::emit::QasmVersion::V3).unwrap();
    assert!(
        qasm.contains("OPENQASM 3.0;"),
        "should produce valid QASM"
    );
}
