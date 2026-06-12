// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! CBOR round-trip fuzz testing for Ehrenfest program deserialization (#725).
//!
//! Three layers of coverage:
//!
//! 1. **Property-based round-trip**: random schema-valid [`EhrenfestProgram`]s
//!    survive `serialize → deserialize → serialize → deserialize` with full
//!    structural equality.
//! 2. **Robustness**: arbitrary and truncated byte sequences must produce a
//!    graceful `CborError`, never a panic.
//! 3. **Regression corpus**: seed files in `tests/cbor_corpus/` (checked in)
//!    pin down edge cases — empty Hamiltonians, zero-qubit systems, maximum
//!    observable count, malformed CBOR.

use std::path::PathBuf;

use proptest::collection::vec;
use proptest::prelude::*;

use afana::cbor::{
    self, CoolingProfile, EhrenfestProgram, EvolutionTime, Hamiltonian, NoiseConstraint,
    Observable, PauliOp, PauliOpEntry, SystemDef,
};

fn to_cbor(program: &EhrenfestProgram) -> Vec<u8> {
    let mut buf = Vec::new();
    ciborium::into_writer(program, &mut buf).expect("CBOR serialization must not fail");
    buf
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cbor_corpus")
}

// ── Proptest strategies for schema-valid programs ────────────────────────────

/// Finite, non-NaN floats so structural equality is well-defined.
fn finite_f64(range: std::ops::Range<f64>) -> impl Strategy<Value = f64> {
    range.prop_filter("finite", |v| v.is_finite())
}

fn pauli_op() -> impl Strategy<Value = PauliOp> {
    prop_oneof![
        Just(PauliOp::I),
        Just(PauliOp::X),
        Just(PauliOp::Y),
        Just(PauliOp::Z),
    ]
}

fn pauli_term() -> impl Strategy<Value = PauliTermArgs> {
    (
        finite_f64(-1e3..1e3),
        vec((0usize..64, pauli_op()), 0..8),
    )
}

type PauliTermArgs = (f64, Vec<(usize, PauliOp)>);

fn observable() -> impl Strategy<Value = Observable> {
    prop_oneof![
        (0usize..64).prop_map(|qubit| Observable::SZ { qubit }),
        (0usize..64).prop_map(|qubit| Observable::SX { qubit }),
        Just(Observable::E),
        vec(0usize..64, 0..6).prop_map(|qubits| Observable::Density { qubits }),
        vec(0u8..2, 0..16).prop_map(|target_state| Observable::F { target_state }),
    ]
}

fn cooling_profile() -> impl Strategy<Value = CoolingProfile> {
    (finite_f64(0.001..300.0), prop::option::of(finite_f64(0.1..1e4))).prop_map(
        |(target_temp_mk, ramp_time_us)| CoolingProfile {
            target_temp_mk,
            ramp_time_us,
        },
    )
}

prop_compose! {
    fn ehrenfest_program()(
        n_qubits in 1usize..128,
        cooling in prop::option::of(cooling_profile()),
        backend_hint in prop::option::of("[a-z0-9_-]{1,24}"),
        terms in vec(pauli_term(), 0..32),
        constant_offset in finite_f64(-1e3..1e3),
        total_us in finite_f64(0.01..1e4),
        steps in 1u32..10_000,
        observables in vec(observable(), 0..16),
        t1_us in finite_f64(1.0..1e6),
        t2_us in finite_f64(1.0..1e6),
        gate_fidelity_min in prop::option::of(finite_f64(0.0..1.0)),
        readout_fidelity_min in prop::option::of(finite_f64(0.0..1.0)),
    ) -> EhrenfestProgram {
        EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits,
                cooling_profile: cooling,
                backend_hint,
            },
            hamiltonian: Hamiltonian {
                terms: terms
                    .into_iter()
                    .map(|(coefficient, paulis)| cbor::PauliTerm {
                        coefficient,
                        paulis: paulis
                            .into_iter()
                            .map(|(qubit, axis)| PauliOpEntry { qubit, axis })
                            .collect(),
                    })
                    .collect(),
                constant_offset,
            },
            evolution: EvolutionTime {
                total_us,
                steps,
                // Computed the same way the schema check recomputes it, so the
                // dt consistency validation holds bit-for-bit.
                dt_us: total_us / steps as f64,
                },
            observables,
            noise: NoiseConstraint {
                t1_us,
                t2_us,
                gate_fidelity_min,
                readout_fidelity_min,
            },
        }
    }
}

// ── Property-based fuzz tests ────────────────────────────────────────────────

proptest! {
    /// deserialize → serialize → deserialize is the identity on valid programs.
    #[test]
    fn roundtrip_is_identity(program in ehrenfest_program()) {
        let first_bytes = to_cbor(&program);
        let decoded = cbor::from_cbor(&first_bytes)
            .expect("schema-valid program must deserialize");
        prop_assert_eq!(&decoded, &program);

        let second_bytes = to_cbor(&decoded);
        let redecoded = cbor::from_cbor(&second_bytes)
            .expect("re-serialized program must deserialize");
        prop_assert_eq!(&redecoded, &program);
        // Canonical encoding: identical structs encode to identical bytes.
        prop_assert_eq!(first_bytes, second_bytes);
    }

    /// Arbitrary byte garbage never panics — it returns a structured error.
    #[test]
    fn arbitrary_bytes_never_panic(bytes in vec(any::<u8>(), 0..512)) {
        // Ok is acceptable (the fuzzer may stumble on a valid encoding);
        // the property under test is the absence of panics.
        let _ = cbor::from_cbor(&bytes);
    }

    /// Truncating a valid program at any point yields an error, not a panic.
    #[test]
    fn truncated_programs_error_gracefully(
        program in ehrenfest_program(),
        cut in 0.0f64..1.0,
    ) {
        let bytes = to_cbor(&program);
        let cut_at = ((bytes.len() as f64) * cut) as usize;
        // Strictly shorter than the full encoding.
        let truncated = &bytes[..cut_at.min(bytes.len().saturating_sub(1))];
        prop_assert!(cbor::from_cbor(truncated).is_err());
    }

    /// Flipping a single byte never panics.
    #[test]
    fn bitflipped_programs_never_panic(
        program in ehrenfest_program(),
        pos in 0.0f64..1.0,
        xor in 1u8..=255,
    ) {
        let mut bytes = to_cbor(&program);
        let idx = ((bytes.len() as f64) * pos) as usize % bytes.len();
        bytes[idx] ^= xor;
        let _ = cbor::from_cbor(&bytes);
    }
}

// ── Regression tests for fuzz-discovered edge cases ──────────────────────────

fn minimal_program() -> EhrenfestProgram {
    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 1,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![cbor::PauliTerm {
                coefficient: 1.0,
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
            }],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 1,
            dt_us: 1.0,
        },
        observables: vec![Observable::E],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

/// Edge case: an empty Hamiltonian (zero terms) is accepted at the CBOR layer
/// and round-trips; semantic rejection is the Trotterization pass's job.
#[test]
fn empty_hamiltonian_roundtrips() {
    let mut program = minimal_program();
    program.hamiltonian.terms.clear();

    let bytes = to_cbor(&program);
    let decoded = cbor::from_cbor(&bytes).expect("empty Hamiltonian must parse");
    assert_eq!(decoded, program);
    assert!(decoded.hamiltonian.terms.is_empty());
}

/// Edge case: a zero-qubit system is rejected with a schema error, not a panic.
#[test]
fn zero_qubit_system_rejected() {
    let mut program = minimal_program();
    program.system.n_qubits = 0;

    let err = cbor::from_cbor(&to_cbor(&program)).unwrap_err();
    assert!(
        err.to_string().contains("n_qubits"),
        "error should mention n_qubits, got: {err}"
    );
}

/// Edge case: maximum observable count (large vector) round-trips intact.
#[test]
fn max_observable_count_roundtrips() {
    let mut program = minimal_program();
    program.observables = (0..4096)
        .map(|i| match i % 5 {
            0 => Observable::SZ { qubit: i % 64 },
            1 => Observable::SX { qubit: i % 64 },
            2 => Observable::E,
            3 => Observable::Density { qubits: vec![i % 64, (i + 1) % 64] },
            _ => Observable::F { target_state: vec![0, 1, 1, 0] },
        })
        .collect();

    let decoded = cbor::from_cbor(&to_cbor(&program)).expect("4096 observables must parse");
    assert_eq!(decoded.observables.len(), 4096);
    assert_eq!(decoded, program);
}

/// Edge case: non-finite coefficients survive the parse layer without panic.
/// (NaN breaks `PartialEq`, so only deserializability is asserted here.)
#[test]
fn non_finite_coefficient_does_not_panic() {
    for coeff in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut program = minimal_program();
        program.hamiltonian.terms[0].coefficient = coeff;
        let _ = cbor::from_cbor(&to_cbor(&program));
    }
}

/// Edge case: a dt_us inconsistent with total_us / steps is a schema error.
#[test]
fn inconsistent_dt_rejected() {
    let mut program = minimal_program();
    program.evolution.dt_us = 0.5; // total_us=1.0, steps=1 → expected dt 1.0

    let err = cbor::from_cbor(&to_cbor(&program)).unwrap_err();
    assert!(err.to_string().contains("dt_us"), "got: {err}");
}

/// The repository's real example programs round-trip with full equality.
#[test]
fn repository_examples_roundtrip() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for name in ["heisenberg.paul", "ising.paul", "rabi.paul"] {
        let path = examples.join(name);
        let original = cbor::from_cbor_file(&path)
            .unwrap_or_else(|e| panic!("{name} must parse: {e}"));
        let decoded = cbor::from_cbor(&to_cbor(&original))
            .unwrap_or_else(|e| panic!("{name} must round-trip: {e}"));
        assert_eq!(decoded, original, "{name} round-trip mismatch");
    }
}

// ── Checked-in corpus ────────────────────────────────────────────────────────

/// Every corpus seed must be handled gracefully: `valid_*` seeds parse Ok,
/// every other seed returns a structured error — and nothing panics.
#[test]
fn corpus_seeds_never_panic() {
    let dir = corpus_dir();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("corpus dir {} missing: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "cbor"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "corpus must contain seed files");

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let result = cbor::from_cbor(&bytes);
        if name.starts_with("valid_") {
            assert!(result.is_ok(), "{name} should parse, got: {result:?}");
        } else {
            assert!(result.is_err(), "{name} should be rejected, got Ok");
        }
    }
}

/// Regenerates the corpus seed files. Run manually with:
/// `cargo test -p afana --test cbor_roundtrip_fuzz regenerate_corpus -- --ignored`
#[test]
#[ignore = "writes corpus seed files; run manually to regenerate"]
fn regenerate_corpus() {
    let dir = corpus_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, bytes: &[u8]| std::fs::write(dir.join(name), bytes).unwrap();

    // Valid: minimal single-qubit program.
    write("valid_minimal.cbor", &to_cbor(&minimal_program()));

    // Valid: every optional field and observable variant populated.
    let full = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 8,
            cooling_profile: Some(CoolingProfile {
                target_temp_mk: 15.0,
                ramp_time_us: Some(250.0),
            }),
            backend_hint: Some("ibm_torino".into()),
        },
        hamiltonian: Hamiltonian {
            terms: vec![
                cbor::PauliTerm {
                    coefficient: 0.25,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::X },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Y },
                        PauliOpEntry { qubit: 2, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 3, axis: PauliOp::I },
                    ],
                },
                cbor::PauliTerm {
                    coefficient: -1.5,
                    paulis: vec![PauliOpEntry { qubit: 7, axis: PauliOp::Z }],
                },
            ],
            constant_offset: -0.75,
        },
        evolution: EvolutionTime { total_us: 2.0, steps: 20, dt_us: 0.1 },
        observables: vec![
            Observable::SZ { qubit: 0 },
            Observable::SX { qubit: 1 },
            Observable::E,
            Observable::Density { qubits: vec![0, 1] },
            Observable::F { target_state: vec![0, 1, 1, 0, 0, 1, 1, 0] },
        ],
        noise: NoiseConstraint {
            t1_us: 120.0,
            t2_us: 80.0,
            gate_fidelity_min: Some(0.995),
            readout_fidelity_min: Some(0.97),
        },
    };
    write("valid_full.cbor", &to_cbor(&full));

    // Valid: empty Hamiltonian (parse-layer edge case).
    let mut empty_h = minimal_program();
    empty_h.hamiltonian.terms.clear();
    write("valid_empty_hamiltonian.cbor", &to_cbor(&empty_h));

    // Invalid: zero qubits (schema check).
    let mut zero_q = minimal_program();
    zero_q.system.n_qubits = 0;
    write("zero_qubits.cbor", &to_cbor(&zero_q));

    // Invalid: unsupported version.
    let mut bad_version = minimal_program();
    bad_version.version = 99;
    write("bad_version.cbor", &to_cbor(&bad_version));

    // Invalid: inconsistent dt_us.
    let mut bad_dt = minimal_program();
    bad_dt.evolution.dt_us = 123.0;
    write("bad_dt.cbor", &to_cbor(&bad_dt));

    // Invalid: truncated valid program.
    let bytes = to_cbor(&minimal_program());
    write("truncated.cbor", &bytes[..bytes.len() / 2]);

    // Invalid: wrong CBOR major type at the root (array, not map).
    let mut arr = Vec::new();
    ciborium::into_writer(&vec![1u8, 2, 3], &mut arr).unwrap();
    write("wrong_major_type.cbor", &arr);

    // Invalid: out-of-range Pauli axis tag.
    let mut bad_axis_doc = Vec::new();
    ciborium::into_writer(
        &serde_json::json!({
            "version": 1,
            "system": { "n_qubits": 1 },
            "hamiltonian": {
                "terms": [ { "coefficient": 1.0, "paulis": [ { "qubit": 0, "axis": 9 } ] } ],
                "constant_offset": 0.0
            },
            "evolution": { "total_us": 1.0, "steps": 1, "dt_us": 1.0 },
            "observables": [],
            "noise": { "t1_us": 100.0, "t2_us": 50.0 }
        }),
        &mut bad_axis_doc,
    )
    .unwrap();
    write("bad_pauli_axis.cbor", &bad_axis_doc);

    // Invalid: empty input.
    write("empty_input.cbor", &[]);
}
