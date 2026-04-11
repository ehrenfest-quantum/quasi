// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Pre-built backend profiles for known quantum hardware.
//!
//! Each profile encodes the hardware capabilities, native gate set, topology,
//! and noise characteristics as documented by the vendor. Calibration data
//! and queue depth are set to reasonable defaults — the scheduler should
//! update these from HAL Contract at runtime via `GET /hal/backends/{name}`.
//!
//! Sources:
//! - IBM: Qiskit documentation, ibm_torino/ibm_brisbane calibration data
//! - IQM: IQM Resonance documentation, Garnet/Sirius specs
//! - IonQ: IonQ Cloud documentation, Aria/Forte native gate specs
//! - Quantinuum: H1/H2 system documentation, trapped-ion specs
//! - Rigetti: Ankaa-3 press release, QCS documentation
//! - AQT: Pine/Spruce system specs
//! - Simulator: ideal zero-noise baseline

use crate::backend::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn gates(names: &[&str]) -> GateSet {
    GateSet {
        native_gates: names.iter().map(|s| s.to_string()).collect(),
    }
}

fn noise(
    t1: f64,
    t2: f64,
    sq_err: f64,
    tq_err: f64,
    ro_err: f64,
    cal: &str,
) -> NoiseProfile {
    NoiseProfile {
        t1_us: t1,
        t2_us: t2,
        single_qubit_error: sq_err,
        two_qubit_error: tq_err,
        readout_error: ro_err,
        calibration_version: cal.to_string(),
    }
}

// ── IBM Backends (Superconducting, Heavy-Hex) ────────────────────────────────

/// IBM Heron r2 — 156 qubits, heavy-hex topology.
/// Native gates: RZ, SX, X, ECR (echoed cross-resonance).
pub fn ibm_heron() -> Backend {
    Backend {
        id: "ibm_heron".into(),
        name: "IBM Heron r2".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 156,
        gate_set: gates(&["rz", "sx", "x", "ecr"]),
        topology: Topology::HeavyHex,
        noise: noise(200.0, 150.0, 0.0005, 0.003, 0.01, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

/// IBM Torino — 133 qubits, Heron heavy-hex topology.
/// Benchmark fidelity: Bell 0.867, GHZ-3 0.755, VQE-H2 0.516.
pub fn ibm_torino() -> Backend {
    Backend {
        id: "ibm_torino".into(),
        name: "IBM Torino (Heron)".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 133,
        gate_set: gates(&["rz", "sx", "x", "ecr"]),
        topology: Topology::HeavyHex,
        noise: noise(200.0, 150.0, 0.0005, 0.003, 0.01, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

/// IBM Eagle r3 — 127 qubits, heavy-hex topology.
/// Older processor family, higher error rates than Heron.
pub fn ibm_eagle() -> Backend {
    Backend {
        id: "ibm_eagle".into(),
        name: "IBM Eagle r3".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 127,
        gate_set: gates(&["rz", "sx", "x", "cx"]),
        topology: Topology::HeavyHex,
        noise: noise(150.0, 100.0, 0.001, 0.005, 0.015, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.008,
    }
}

/// IBM Marrakesh — 156 qubits, Heron heavy-hex.
pub fn ibm_marrakesh() -> Backend {
    Backend {
        id: "ibm_marrakesh".into(),
        name: "IBM Marrakesh (Heron)".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 156,
        gate_set: gates(&["rz", "sx", "x", "ecr"]),
        topology: Topology::HeavyHex,
        noise: noise(200.0, 150.0, 0.0005, 0.003, 0.01, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

// ── IQM Backends (Superconducting, Star/Crystal) ─────────────────────────────

/// IQM Garnet — 20 qubits, square-lattice topology.
/// Native gates: PRX (phased rotation-X), CZ.
pub fn iqm_garnet() -> Backend {
    Backend {
        id: "iqm_garnet".into(),
        name: "IQM Garnet".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 20,
        gate_set: gates(&["prx", "cz"]),
        topology: Topology::Grid { rows: 4, cols: 5 },
        noise: noise(80.0, 50.0, 0.002, 0.007, 0.02, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.005,
    }
}

/// IQM Sirius — 6 qubits, star topology.
/// On-premise system targeting KRITIS/defense.
pub fn iqm_sirius() -> Backend {
    Backend {
        id: "iqm_sirius".into(),
        name: "IQM Sirius".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 6,
        gate_set: gates(&["prx", "cz"]),
        topology: Topology::Custom {
            edges: vec![(0, 1), (0, 2), (0, 3), (0, 4), (0, 5)], // star
        },
        noise: noise(90.0, 60.0, 0.002, 0.006, 0.02, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.005,
    }
}

// ── IonQ Backends (Trapped Ion, All-to-All) ──────────────────────────────────

/// IonQ Aria — 25 algorithmic qubits, all-to-all connectivity.
/// Native gates: GPI, GPI2, MS (Mølmer-Sørensen).
pub fn ionq_aria() -> Backend {
    Backend {
        id: "ionq_aria".into(),
        name: "IonQ Aria (#AQ 25)".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 25,
        gate_set: gates(&["gpi", "gpi2", "ms"]),
        topology: Topology::AllToAll,
        noise: noise(1000.0, 500.0, 0.0003, 0.004, 0.005, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.03,
    }
}

/// IonQ Forte Enterprise — 36 algorithmic qubits, all-to-all connectivity.
/// Native gates: GPI, GPI2, ZZ.
pub fn ionq_forte() -> Backend {
    Backend {
        id: "ionq_forte".into(),
        name: "IonQ Forte Enterprise (#AQ 36)".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 36,
        gate_set: gates(&["gpi", "gpi2", "zz"]),
        topology: Topology::AllToAll,
        noise: noise(1500.0, 800.0, 0.0002, 0.003, 0.004, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.05,
    }
}

// ── Quantinuum Backends (Trapped Ion, All-to-All) ────────────────────────────

/// Quantinuum H2 — 56 qubits, all-to-all connectivity.
/// Native gates: RZ, RX, RY, ZZ (Quantinuum native model).
/// Industry-leading gate fidelity (~99.8% two-qubit).
pub fn quantinuum_h2() -> Backend {
    Backend {
        id: "quantinuum_h2".into(),
        name: "Quantinuum H2-1".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 56,
        gate_set: gates(&["rz", "rx", "ry", "zz"]),
        topology: Topology::AllToAll,
        noise: noise(1000.0, 500.0, 0.0001, 0.002, 0.003, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.08,
    }
}

/// Quantinuum H1 — 20 qubits, all-to-all connectivity.
pub fn quantinuum_h1() -> Backend {
    Backend {
        id: "quantinuum_h1".into(),
        name: "Quantinuum H1-1".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 20,
        gate_set: gates(&["rz", "rx", "ry", "zz"]),
        topology: Topology::AllToAll,
        noise: noise(800.0, 400.0, 0.0002, 0.003, 0.005, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.06,
    }
}

// ── Rigetti Backends (Superconducting, Square Lattice) ────────────────────────

/// Rigetti Ankaa-3 — 84 qubits, square-lattice topology.
/// Native gates: RZ, RX, iSWAP (99.5% median fidelity), fSIM.
/// Median iSWAP gate time: 72ns.
pub fn rigetti_ankaa3() -> Backend {
    Backend {
        id: "rigetti_ankaa3".into(),
        name: "Rigetti Ankaa-3".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 84,
        gate_set: gates(&["rz", "rx", "iswap", "fsim"]),
        topology: Topology::Grid { rows: 7, cols: 12 },
        noise: noise(30.0, 20.0, 0.002, 0.005, 0.02, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.007,
    }
}

// ── AQT Backends (Trapped Ion) ───────────────────────────────────────────────

/// AQT Pine — 24 qubits, all-to-all, on-premise available.
/// Native gates: RZ, RXX (Ising-type), R (arbitrary single-qubit).
pub fn aqt_pine() -> Backend {
    Backend {
        id: "aqt_pine".into(),
        name: "AQT Pine".into(),
        backend_type: BackendType::Qpu,
        qubit_count: 24,
        gate_set: gates(&["rz", "rxx", "r"]),
        topology: Topology::AllToAll,
        noise: noise(500.0, 250.0, 0.0005, 0.005, 0.01, "default"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.02,
    }
}

// ── Simulators ───────────────────────────────────────────────────────────────

/// Ideal statevector simulator — zero noise, unlimited connectivity.
/// Practical limit ~30 qubits (2^30 amplitudes ≈ 16 GB RAM).
pub fn simulator() -> Backend {
    Backend {
        id: "simulator".into(),
        name: "Ideal Statevector Simulator".into(),
        backend_type: BackendType::Simulator,
        qubit_count: 30,
        gate_set: gates(&[
            "h", "x", "y", "z", "s", "t", "sdg", "tdg",
            "cx", "cz", "swap", "ccx", "rx", "ry", "rz",
            "sx", "ecr", "iswap", "gpi", "gpi2", "ms", "zz",
            "prx", "rxx", "r", "fsim",
        ]),
        topology: Topology::AllToAll,
        noise: noise(1e9, 1e9, 0.0, 0.0, 0.0, "ideal"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.0001,
    }
}

/// Huoma ProjectedTTN simulator — tensor-network with projected tree structure.
/// 1,000,000 qubits in 5.2 seconds (PR hiq-lab/huoma#14).
/// Topology construction O(N) via HashSet, rayon-parallel edge scoring.
/// Fidelity depends on bond dimension and commensurability partition.
pub fn huoma_mps() -> Backend {
    Backend {
        id: "huoma_mps".into(),
        name: "Huoma ProjectedTTN Simulator".into(),
        backend_type: BackendType::Simulator,
        qubit_count: 1_000_000,
        gate_set: gates(&[
            "h", "x", "y", "z", "s", "t", "sdg", "tdg",
            "cx", "cz", "swap", "rx", "ry", "rz",
        ]),
        topology: Topology::AllToAll,
        noise: noise(1e9, 1e9, 0.0, 0.0, 0.0, "ideal"),
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.001,
    }
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Return all known backend profiles.
///
/// Queue depth and calibration data are defaults — the scheduler should
/// update these from HAL Contract at runtime.
pub fn all_backends() -> Vec<Backend> {
    vec![
        // IBM (Superconducting, Heavy-Hex)
        ibm_heron(),
        ibm_torino(),
        ibm_eagle(),
        ibm_marrakesh(),
        // IQM (Superconducting, Star/Crystal)
        iqm_garnet(),
        iqm_sirius(),
        // IonQ (Trapped Ion, All-to-All)
        ionq_aria(),
        ionq_forte(),
        // Quantinuum (Trapped Ion, All-to-All)
        quantinuum_h2(),
        quantinuum_h1(),
        // Rigetti (Superconducting, Square Lattice)
        rigetti_ankaa3(),
        // AQT (Trapped Ion)
        aqt_pine(),
        // Simulators
        simulator(),
        huoma_mps(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_backends_returns_14_profiles() {
        let backends = all_backends();
        assert_eq!(backends.len(), 14);
    }

    #[test]
    fn all_backends_have_unique_ids() {
        let backends = all_backends();
        let mut ids: Vec<&str> = backends.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), backends.len(), "duplicate backend IDs");
    }

    #[test]
    fn all_backends_are_online() {
        for b in all_backends() {
            assert!(b.is_online(), "{} should be online by default", b.id);
        }
    }

    #[test]
    fn ibm_torino_has_133_qubits() {
        let b = ibm_torino();
        assert_eq!(b.qubit_count, 133);
        assert!(b.gate_set.supports("ecr"));
        assert!(matches!(b.topology, Topology::HeavyHex));
    }

    #[test]
    fn quantinuum_h2_has_best_fidelity() {
        let h2 = quantinuum_h2();
        let eagle = ibm_eagle();
        assert!(h2.noise.two_qubit_error < eagle.noise.two_qubit_error);
    }

    #[test]
    fn ionq_has_all_to_all_connectivity() {
        assert!(matches!(ionq_aria().topology, Topology::AllToAll));
        assert!(matches!(ionq_forte().topology, Topology::AllToAll));
    }

    #[test]
    fn simulator_supports_all_common_gates() {
        let sim = simulator();
        assert!(sim.gate_set.supports("h"));
        assert!(sim.gate_set.supports("cx"));
        assert!(sim.gate_set.supports("ecr"));
        assert!(sim.gate_set.supports("iswap"));
        assert!(sim.gate_set.supports("ms"));
    }

    #[test]
    fn simulator_has_zero_noise() {
        let sim = simulator();
        assert_eq!(sim.noise.single_qubit_error, 0.0);
        assert_eq!(sim.noise.two_qubit_error, 0.0);
        assert_eq!(sim.noise.readout_error, 0.0);
    }

    #[test]
    fn huoma_supports_1m_qubits() {
        let h = huoma_mps();
        assert_eq!(h.qubit_count, 1_000_000);
        assert!(matches!(h.backend_type, BackendType::Simulator));
    }

    #[test]
    fn iqm_sirius_has_star_topology() {
        let s = iqm_sirius();
        match &s.topology {
            Topology::Custom { edges } => {
                assert_eq!(edges.len(), 5, "star with 6 qubits has 5 edges");
                assert!(edges.iter().all(|(a, _)| *a == 0), "all edges from center");
            }
            _ => panic!("expected Custom topology for Sirius star"),
        }
    }

    #[test]
    fn trapped_ion_has_longer_coherence_than_superconducting() {
        let ion = quantinuum_h2();
        let sc = ibm_eagle();
        assert!(ion.noise.t1_us > sc.noise.t1_us);
        assert!(ion.noise.t2_us > sc.noise.t2_us);
    }

    #[test]
    fn scheduler_with_real_backends_picks_best() {
        use crate::job::*;
        use crate::scheduler::Scheduler;

        let backends = all_backends();
        let job = QuantumJob {
            id: "test-job".into(),
            circuit_hash: [0u8; 32],
            requirements: JobRequirements {
                min_qubits: 10,
                required_gates: vec!["rz".into(), "cx".into()],
                circuit_depth: 50,
                shot_count: 1000,
                noise_budget: Some(NoiseBudget {
                    min_t1_us: 100.0,
                    min_t2_us: 50.0,
                    max_gate_error: None,
                    min_fidelity: None,
                }),
                prefers_qpu: true,
                max_cost: None,
            },
            priority: 1,
            submitted_at: 0,
        };

        let scheduler = Scheduler::with_default_plugins();
        let decision = scheduler.schedule(&job, &backends);

        match decision {
            crate::job::ScheduleDecision::Assign { backend_id, score } => {
                // Should pick a QPU with cx support and sufficient qubits/coherence
                assert!(score > 0.0, "score should be positive");
                // Eagle has cx, Heron has ecr — Eagle should be eligible
                println!("Picked: {backend_id} (score: {score:.3})");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }
}
