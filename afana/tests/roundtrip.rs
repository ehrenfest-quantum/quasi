// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Integration tests: CBOR roundtrip — deserialize, serialize, deserialize again.

use std::path::PathBuf;

use afana::cbor;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("{name}.paul"))
}

fn roundtrip(name: &str) {
    let original = cbor::from_cbor_file(&example_path(name)).unwrap();

    // Serialize back to CBOR.
    let mut buf = Vec::new();
    ciborium::into_writer(&original, &mut buf).unwrap();

    // Deserialize again.
    let decoded = cbor::from_cbor(&buf).unwrap();

    // Structural equality.
    assert_eq!(original.version, decoded.version);
    assert_eq!(original.system.n_qubits, decoded.system.n_qubits);
    assert_eq!(original.system.backend_hint, decoded.system.backend_hint);
    assert_eq!(original.hamiltonian.terms.len(), decoded.hamiltonian.terms.len());
    assert_eq!(original.hamiltonian.constant_offset, decoded.hamiltonian.constant_offset);
    assert_eq!(original.evolution.total_us, decoded.evolution.total_us);
    assert_eq!(original.evolution.steps, decoded.evolution.steps);
    assert_eq!(original.evolution.dt_us, decoded.evolution.dt_us);
    assert_eq!(original.observables, decoded.observables);
    assert_eq!(original.noise, decoded.noise);

    // Deep equality of Hamiltonian terms.
    for (a, b) in original.hamiltonian.terms.iter().zip(decoded.hamiltonian.terms.iter()) {
        assert_eq!(a.coefficient, b.coefficient);
        assert_eq!(a.paulis.len(), b.paulis.len());
        for (pa, pb) in a.paulis.iter().zip(b.paulis.iter()) {
            assert_eq!(pa.qubit, pb.qubit);
            assert_eq!(pa.axis, pb.axis);
        }
    }
}

#[test]
fn roundtrip_rabi() {
    roundtrip("rabi");
}

#[test]
fn roundtrip_ising() {
    roundtrip("ising");
}

#[test]
fn roundtrip_heisenberg() {
    roundtrip("heisenberg");
}
