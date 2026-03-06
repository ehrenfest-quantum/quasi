// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! CBOR deserialization for Ehrenfest binary programs.
//!
//! Ehrenfest programs are CBOR documents conforming to the schema in
//! `spec/ehrenfest-v0.1.cddl`. This module deserializes them into typed Rust
//! structs that represent the Hamiltonian, observables, and noise constraints.
//!
//! These structs are the input to Afana's Trotterization pass, which derives
//! a gate sequence and produces an [`EhrenfestAst`] for QASM emission.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::CborError;

// ── CBOR schema types ────────────────────────────────────────────────────────

/// A Pauli axis: I(0), X(1), Y(2), Z(3).
///
/// Serializes as an integer matching the CDDL PauliAxis definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauliOp {
    I,
    X,
    Y,
    Z,
}

impl PauliOp {
    fn from_u64(v: u64) -> Option<Self> {
        match v {
            0 => Some(Self::I),
            1 => Some(Self::X),
            2 => Some(Self::Y),
            3 => Some(Self::Z),
            _ => None,
        }
    }

    fn to_u64(self) -> u64 {
        match self {
            Self::I => 0,
            Self::X => 1,
            Self::Y => 2,
            Self::Z => 3,
        }
    }
}

impl Serialize for PauliOp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.to_u64())
    }
}

impl<'de> Deserialize<'de> for PauliOp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = u64::deserialize(deserializer)?;
        Self::from_u64(v).ok_or_else(|| serde::de::Error::custom(format!("invalid PauliAxis: {v}")))
    }
}

/// A single Pauli operator on a specific qubit.
///
/// CDDL: `PauliOp = { "qubit": uint, "axis": PauliAxis }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PauliOpEntry {
    pub qubit: usize,
    pub axis: PauliOp,
}

/// A single Pauli term in a Hamiltonian: coefficient × tensor product of Pauli operators.
///
/// CDDL: `PauliTerm = { "coefficient": float, "paulis": [* PauliOp] }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PauliTerm {
    pub coefficient: f64,
    pub paulis: Vec<PauliOpEntry>,
}

/// Hamiltonian expressed as a sum of Pauli terms plus a constant offset.
///
/// CDDL: `Hamiltonian = { "terms": [+ PauliTerm], "constant_offset": float }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hamiltonian {
    pub terms: Vec<PauliTerm>,
    pub constant_offset: f64,
}

/// Evolution time and Trotter step parameters.
///
/// CDDL: `EvolutionTime = { "total_us": float, "steps": uint, "dt_us": float }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionTime {
    pub total_us: f64,
    pub steps: u32,
    pub dt_us: f64,
}

/// Noise constraints — compile-time type check against hardware capabilities.
///
/// CDDL: t1_us and t2_us are required; gate_fidelity_min and readout_fidelity_min optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseConstraint {
    pub t1_us: f64,
    pub t2_us: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_fidelity_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readout_fidelity_min: Option<f64>,
}

/// An observable to measure after time evolution.
///
/// CDDL: `Observable = SigmaZ / SigmaX / Energy / Density / Fidelity`
/// Uses `"type"` field as discriminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Observable {
    /// σᶻ expectation on a single qubit.
    SZ { qubit: usize },
    /// σˣ expectation on a single qubit.
    SX { qubit: usize },
    /// Energy ⟨ψ|H|ψ⟩.
    E,
    /// Reduced density matrix on a subset of qubits.
    #[serde(rename = "rho")]
    Density { qubits: Vec<usize> },
    /// State fidelity against a reference state.
    F { target_state: Vec<u8> },
}

/// Hardware cooling requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoolingProfile {
    pub target_temp_mk: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ramp_time_us: Option<f64>,
}

/// Physical system definition.
///
/// CDDL: `PhysicalContext = { "n_qubits": uint, ? "cooling_profile": ..., ? "backend_hint": tstr }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemDef {
    pub n_qubits: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooling_profile: Option<CoolingProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_hint: Option<String>,
}

/// Top-level Ehrenfest binary program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EhrenfestProgram {
    pub version: u32,
    pub system: SystemDef,
    pub hamiltonian: Hamiltonian,
    pub evolution: EvolutionTime,
    pub observables: Vec<Observable>,
    pub noise: NoiseConstraint,
}

// ── Deserialization ──────────────────────────────────────────────────────────

/// Deserialize an Ehrenfest binary program from CBOR bytes.
pub fn from_cbor(bytes: &[u8]) -> Result<EhrenfestProgram, CborError> {
    let program: EhrenfestProgram =
        ciborium::from_reader(bytes).map_err(|e| CborError::Decode(e.to_string()))?;

    // Schema validation.
    if program.version != 1 {
        return Err(CborError::Schema(format!(
            "unsupported Ehrenfest version: {} (expected 1)",
            program.version
        )));
    }
    if program.system.n_qubits == 0 {
        return Err(CborError::Schema("n_qubits must be >= 1".into()));
    }
    // dt_us consistency check.
    let expected_dt = program.evolution.total_us / program.evolution.steps as f64;
    if (program.evolution.dt_us - expected_dt).abs() > 1e-9 {
        return Err(CborError::Schema(format!(
            "dt_us ({}) != total_us / steps ({})",
            program.evolution.dt_us, expected_dt
        )));
    }

    Ok(program)
}

/// Deserialize an Ehrenfest binary program from a CBOR file.
pub fn from_cbor_file(path: &std::path::Path) -> Result<EhrenfestProgram, CborError> {
    let bytes = std::fs::read(path)?;
    from_cbor(&bytes)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_program() -> EhrenfestProgram {
        EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 2,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::Z },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Z },
                    ],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us: 1.0,
                steps: 10,
                dt_us: 0.1,
            },
            observables: vec![Observable::SZ { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: Some(0.99),
                readout_fidelity_min: None,
            },
        }
    }

    #[test]
    fn cbor_roundtrip() {
        let program = make_test_program();

        // Serialize to CBOR.
        let mut buf = Vec::new();
        ciborium::into_writer(&program, &mut buf).unwrap();

        // Deserialize back.
        let decoded = from_cbor(&buf).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.system.n_qubits, 2);
        assert_eq!(decoded.hamiltonian.terms.len(), 1);
        assert_eq!(decoded.hamiltonian.terms[0].coefficient, 0.5);
        assert_eq!(decoded.evolution.steps, 10);
        assert_eq!(decoded.evolution.total_us, 1.0);
        assert_eq!(decoded.evolution.dt_us, 0.1);
        assert_eq!(decoded.hamiltonian.constant_offset, 0.0);
    }

    #[test]
    fn cbor_rejects_bad_version() {
        let mut program = make_test_program();
        program.version = 99;

        let mut buf = Vec::new();
        ciborium::into_writer(&program, &mut buf).unwrap();

        let err = from_cbor(&buf).unwrap_err();
        assert!(err.to_string().contains("unsupported Ehrenfest version"));
    }

    #[test]
    fn pauli_op_serde_integers() {
        // PauliOp should serialize as integers 0-3.
        let entry = PauliOpEntry { qubit: 0, axis: PauliOp::X };
        let mut buf = Vec::new();
        ciborium::into_writer(&entry, &mut buf).unwrap();
        let decoded: PauliOpEntry = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(decoded.axis, PauliOp::X);
        assert_eq!(decoded.qubit, 0);
    }

    #[test]
    fn observable_tagged_enum() {
        let obs = vec![
            Observable::SZ { qubit: 0 },
            Observable::SX { qubit: 1 },
            Observable::E,
        ];
        let mut buf = Vec::new();
        ciborium::into_writer(&obs, &mut buf).unwrap();
        let decoded: Vec<Observable> = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(decoded, obs);
    }
}
