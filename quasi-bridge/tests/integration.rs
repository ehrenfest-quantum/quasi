// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for quasi-bridge.

use quasi_bridge::basis;
use quasi_bridge::ehrenfest_mol::{self, Accuracy};
use quasi_bridge::integrals;
use quasi_bridge::jordan_wigner;
use quasi_bridge::molecule;
use quasi_bridge::partition;
use quasi_bridge::postprocess;
use quasi_bridge::rhf;

// ── Test 1: Ehrenfest CBOR round-trip ──────────────────────────────────────

#[test]
fn ehrenfest_cbor_roundtrip() {
    let terms = vec![
        (0.5, "ZIII".to_string()),
        (-0.3, "IZII".to_string()),
        (0.1, "XXII".to_string()),
        (0.1, "YYII".to_string()),
        (-0.8, "IIII".to_string()),
    ];
    let program = ehrenfest_mol::build_ehrenfest_program(&terms, 4, Accuracy::Chemical);
    let cbor = ehrenfest_mol::to_cbor(&program);
    let decoded = afana::cbor::from_cbor(&cbor).unwrap();

    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.system.n_qubits, 4);
    assert_eq!(decoded.hamiltonian.terms.len(), program.hamiltonian.terms.len());
    assert!((decoded.hamiltonian.constant_offset - (-0.8)).abs() < 1e-10);
}

// ── Test 2: H2 integrals ──────────────────────────────────────────────────

#[test]
fn h2_integrals_sanity() {
    let atoms = molecule::from_smiles("[H][H]").unwrap();
    let bf = basis::build_basis(&atoms, "sto-3g").unwrap();
    assert_eq!(bf.len(), 2);

    let ints = integrals::compute_integrals(&atoms, &bf);

    // Overlap: diagonal should be ~1.0
    assert!(
        (ints.overlap[0][0] - 1.0).abs() < 0.01,
        "S(0,0) = {}",
        ints.overlap[0][0]
    );
    assert!(
        (ints.overlap[1][1] - 1.0).abs() < 0.01,
        "S(1,1) = {}",
        ints.overlap[1][1]
    );

    // Off-diagonal overlap should be positive and < 1
    assert!(ints.overlap[0][1] > 0.0 && ints.overlap[0][1] < 1.0);

    // Nuclear repulsion = 1/1.4
    assert!((ints.nuclear_repulsion - 1.0 / 1.4).abs() < 1e-10);
}

// ── Test 3: H2 RHF energy ────────────────────────────────────────────────

#[test]
fn h2_rhf_converges() {
    let atoms = molecule::from_smiles("[H][H]").unwrap();
    let bf = basis::build_basis(&atoms, "sto-3g").unwrap();
    let ints = integrals::compute_integrals(&atoms, &bf);
    let result = rhf::rhf(&ints, 2).unwrap();

    // RHF/STO-3G for H2 at 1.4 bohr ≈ -1.1175 Ha
    assert!(
        (result.energy - (-1.1175)).abs() < 0.01,
        "H2 RHF energy = {:.6}",
        result.energy
    );
    assert_eq!(result.n_occupied, 1);
    assert!(result.iterations < 50);
}

// ── Test 4: Jordan-Wigner on H2 ──────────────────────────────────────────

#[test]
fn h2_jordan_wigner_produces_pauli_terms() {
    let atoms = molecule::from_smiles("[H][H]").unwrap();
    let bf = basis::build_basis(&atoms, "sto-3g").unwrap();
    let ints = integrals::compute_integrals(&atoms, &bf);
    let rhf_result = rhf::rhf(&ints, 2).unwrap();

    let n = bf.len();
    let h_core_ao: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| ints.kinetic[i][j] + ints.nuclear[i][j]).collect())
        .collect();
    let h_mo = rhf::ao_to_mo_one(&h_core_ao, &rhf_result.mo_coefficients);
    let eri_mo = rhf::ao_to_mo_two(&ints, &rhf_result.mo_coefficients);

    let terms = jordan_wigner::jordan_wigner_transform(&h_mo, &eri_mo, n, ints.nuclear_repulsion);

    // H2 in STO-3G → 4 qubits, should produce multiple Pauli terms
    assert!(!terms.is_empty(), "should have Pauli terms");
    // All strings should be length 4
    for (_, s) in &terms {
        assert_eq!(s.len(), 4, "Pauli string length should be 4 (qubits), got '{s}'");
    }
}

// ── Test 5: Afana accepts molecular Ehrenfest ─────────────────────────────

#[test]
fn afana_compiles_molecular_ehrenfest() {
    let terms = vec![
        (0.5, "ZI".to_string()),
        (-0.3, "IZ".to_string()),
        (0.1, "XX".to_string()),
        (0.1, "YY".to_string()),
    ];
    let program = ehrenfest_mol::build_ehrenfest_program(&terms, 2, Accuracy::Chemical);
    let cbor = ehrenfest_mol::to_cbor(&program);
    let parsed = afana::cbor::from_cbor(&cbor).unwrap();

    let (ast, stats) =
        afana::trotter::trotterize_with_stats(&parsed, afana::trotter::TrotterOrder::Second)
            .unwrap();

    assert!(!ast.gates.is_empty(), "should generate gates");
    assert!(stats.total_gates > 0);

    let qasm = afana::emit::emit_qasm(&ast, afana::emit::QasmVersion::V3).unwrap();
    assert!(qasm.contains("OPENQASM 3.0;"));
}

// ── Test 6: Solvayeur classical fallback ──────────────────────────────────

#[test]
fn solvayeur_classical_fallback() {
    use quasi_scheduler::backend::*;
    use quasi_solvayeur::atw::Solvayeur;
    use quasi_solvayeur::bias::Reward;

    let backends = vec![
        Backend {
            capabilities: Capabilities::simulator(30),
            backend_type: BackendType::Simulator,
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: 0.0001,
        },
        Backend {
            capabilities: Capabilities::simulator(100),
            backend_type: BackendType::Simulator,
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: 0.001,
        },
    ];

    let mut kernel = Solvayeur::new(&backends);
    let result = kernel.decide().unwrap();

    assert!(result.backend_index < 2);
    assert!(!result.ran_on_qpu);
    assert!(result.gate_count > 0);

    // Observe a reward and verify bias updates
    let reward = Reward {
        fidelity: 0.99,
        latency_ms: 1.0,
        cost: 0.0001,
    };
    kernel.observe(&result, reward);
    assert_eq!(kernel.epochs(), 1);
}

// ── Test 9: Cache key determinism ─────────────────────────────────────────

#[test]
fn cache_key_deterministic() {
    use std::collections::BTreeMap;
    let cbor = vec![1u8, 2, 3, 4];
    let k1 = quasi_cache::CacheKey::build(&cbor, "huoma", &BTreeMap::new(), "v1");
    let k2 = quasi_cache::CacheKey::build(&cbor, "huoma", &BTreeMap::new(), "v1");
    assert_eq!(k1, k2);
}

// ── Test 10: Garm JSON import ─────────────────────────────────────────────

#[test]
fn garm_json_import() {
    let json = std::fs::read_to_string("testdata/garm_aspirin_analysis.json").unwrap();
    let part = partition::from_garm_json(&json, 30, 68).unwrap();

    assert_eq!(part.active, vec![15, 16, 17, 18, 19, 20]);
    assert_eq!(part.n_electrons_active, 6);
    assert!(part.method.contains("garm_sinc2"));
}

// ── Test: Exact diag gives correct eigenvalue ─────────────────────────────

#[test]
fn exact_diag_simple() {
    // H = -Z: eigenvalues are -1, +1. Ground state = -1.
    let terms = vec![(-1.0, "Z".to_string())];
    let e0 = postprocess::ground_state_energy(&terms, 1).unwrap();
    assert!((e0 - (-1.0)).abs() < 1e-10, "e0 = {e0}");
}
