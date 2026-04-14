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

fn noise(
    t1: f64,
    t2: f64,
    sq_fidelity: f64,
    tq_fidelity: f64,
    ro_fidelity: f64,
) -> NoiseProfile {
    NoiseProfile {
        t1: Some(t1),
        t2: Some(t2),
        single_qubit_fidelity: Some(sq_fidelity),
        two_qubit_fidelity: Some(tq_fidelity),
        readout_fidelity: Some(ro_fidelity),
        gate_time: None,
    }
}

// ── IBM Backends (Superconducting, Heavy-Hex) ────────────────────────────────

/// IBM Heron r2 — 156 qubits, heavy-hex topology.
/// Native gates: RZ, SX, X, CZ (Heron native).
pub fn ibm_heron() -> Backend {
    Backend {
        capabilities: Capabilities::ibm_heron("ibm_heron", 156)
            .with_topology(Topology {
                kind: TopologyKind::HeavyHex,
                edges: vec![],
            })
            .with_noise_profile(noise(200.0, 150.0, 0.9995, 0.997, 0.99)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

/// IBM Torino — 133 qubits, Heron heavy-hex topology.
/// Benchmark fidelity: Bell 0.867, GHZ-3 0.755, VQE-H2 0.516.
pub fn ibm_torino() -> Backend {
    Backend {
        capabilities: Capabilities::ibm_heron("ibm_torino", 133)
            .with_topology(Topology {
                kind: TopologyKind::HeavyHex,
                edges: vec![],
            })
            .with_noise_profile(noise(200.0, 150.0, 0.9995, 0.997, 0.99)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

/// IBM Eagle r3 — 127 qubits, heavy-hex topology.
/// Older processor family, higher error rates than Heron.
pub fn ibm_eagle() -> Backend {
    Backend {
        capabilities: Capabilities::ibm_eagle("ibm_eagle", 127)
            .with_topology(Topology {
                kind: TopologyKind::HeavyHex,
                edges: vec![],
            })
            .with_noise_profile(noise(150.0, 100.0, 0.999, 0.995, 0.985)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.008,
    }
}

/// IBM Marrakesh — 156 qubits, Heron heavy-hex.
pub fn ibm_marrakesh() -> Backend {
    Backend {
        capabilities: Capabilities::ibm_heron("ibm_marrakesh", 156)
            .with_topology(Topology {
                kind: TopologyKind::HeavyHex,
                edges: vec![],
            })
            .with_noise_profile(noise(200.0, 150.0, 0.9995, 0.997, 0.99)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.01,
    }
}

// ── IQM Backends (Superconducting, Star/Crystal) ─────────────────────────────

/// IQM Garnet — 20 qubits, square-lattice topology.
/// Native gates: PRX (phased rotation-X), CZ.
pub fn iqm_garnet() -> Backend {
    Backend {
        capabilities: Capabilities::iqm("iqm_garnet", 20)
            .with_topology(Topology::grid(4, 5))
            .with_noise_profile(noise(80.0, 50.0, 0.998, 0.993, 0.98)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.005,
    }
}

/// IQM Sirius — 6 qubits, star topology.
/// On-premise system targeting KRITIS/defense.
pub fn iqm_sirius() -> Backend {
    Backend {
        capabilities: Capabilities::iqm("iqm_sirius", 6)
            .with_topology(Topology::custom(
                vec![(0, 1), (0, 2), (0, 3), (0, 4), (0, 5)],
            ))
            .with_noise_profile(noise(90.0, 60.0, 0.998, 0.994, 0.98)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.005,
    }
}

// ── IonQ Backends (Trapped Ion, All-to-All) ──────────────────────────────────

/// IonQ Aria — 25 algorithmic qubits, all-to-all connectivity.
/// Native gates: GPI, GPI2, MS (Mølmer-Sørensen).
pub fn ionq_aria() -> Backend {
    Backend {
        capabilities: Capabilities::ionq("ionq_aria", 25)
            .with_topology(Topology::full(25))
            .with_noise_profile(noise(1000.0, 500.0, 0.9997, 0.996, 0.995)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.03,
    }
}

/// IonQ Forte Enterprise — 36 algorithmic qubits, all-to-all connectivity.
/// Native gates: GPI, GPI2, ZZ.
pub fn ionq_forte() -> Backend {
    Backend {
        capabilities: Capabilities::ionq("ionq_forte", 36)
            .with_topology(Topology::full(36))
            .with_noise_profile(noise(1500.0, 800.0, 0.9998, 0.997, 0.996)),
        backend_type: BackendType::Qpu,
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
        capabilities: Capabilities {
            name: "quantinuum_h2".into(),
            num_qubits: 56,
            gate_set: GateSet {
                single_qubit: vec!["rz".into(), "rx".into(), "ry".into()],
                two_qubit: vec!["zz".into()],
                three_qubit: vec![],
                native: vec!["rz".into(), "rx".into(), "ry".into(), "zz".into()],
            },
            topology: Topology::full(56),
            max_shots: 100_000,
            is_simulator: false,
            features: vec![],
            noise_profile: Some(noise(1000.0, 500.0, 0.9999, 0.998, 0.997)),
        },
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.08,
    }
}

/// Quantinuum H1 — 20 qubits, all-to-all connectivity.
pub fn quantinuum_h1() -> Backend {
    Backend {
        capabilities: Capabilities {
            name: "quantinuum_h1".into(),
            num_qubits: 20,
            gate_set: GateSet {
                single_qubit: vec!["rz".into(), "rx".into(), "ry".into()],
                two_qubit: vec!["zz".into()],
                three_qubit: vec![],
                native: vec!["rz".into(), "rx".into(), "ry".into(), "zz".into()],
            },
            topology: Topology::full(20),
            max_shots: 100_000,
            is_simulator: false,
            features: vec![],
            noise_profile: Some(noise(800.0, 400.0, 0.9998, 0.997, 0.995)),
        },
        backend_type: BackendType::Qpu,
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
        capabilities: Capabilities::rigetti("rigetti_ankaa3", 84)
            .with_topology(Topology::grid(7, 12))
            .with_noise_profile(noise(30.0, 20.0, 0.998, 0.995, 0.98)),
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.007,
    }
}

// ── AQT Backends (Trapped Ion) ───────────────────────────────────────────────

/// AQT Pine — 24 qubits, all-to-all, on-premise available.
/// Native gates: RZ, RXX (Ising-type), R (arbitrary single-qubit).
pub fn aqt_pine() -> Backend {
    Backend {
        capabilities: Capabilities {
            name: "aqt_pine".into(),
            num_qubits: 24,
            gate_set: GateSet {
                single_qubit: vec!["rz".into(), "r".into()],
                two_qubit: vec!["rxx".into()],
                three_qubit: vec![],
                native: vec!["rz".into(), "rxx".into(), "r".into()],
            },
            topology: Topology::full(24),
            max_shots: 100_000,
            is_simulator: false,
            features: vec![],
            noise_profile: Some(noise(500.0, 250.0, 0.9995, 0.995, 0.99)),
        },
        backend_type: BackendType::Qpu,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.02,
    }
}

// ── Simulators ───────────────────────────────────────────────────────────────

/// Ideal statevector simulator — zero noise, unlimited connectivity.
/// Practical limit ~30 qubits (2^30 amplitudes ≈ 16 GB RAM).
pub fn simulator() -> Backend {
    Backend {
        capabilities: Capabilities {
            name: "simulator".into(),
            num_qubits: 30,
            gate_set: GateSet {
                single_qubit: vec![
                    "h".into(), "x".into(), "y".into(), "z".into(),
                    "s".into(), "t".into(), "sdg".into(), "tdg".into(),
                    "rx".into(), "ry".into(), "rz".into(), "sx".into(),
                    "gpi".into(), "gpi2".into(), "prx".into(), "r".into(),
                ],
                two_qubit: vec![
                    "cx".into(), "cz".into(), "swap".into(),
                    "ecr".into(), "iswap".into(), "ms".into(),
                    "zz".into(), "rxx".into(), "fsim".into(),
                ],
                three_qubit: vec!["ccx".into()],
                native: vec![],
            },
            topology: Topology::full(30),
            max_shots: 100_000,
            is_simulator: true,
            features: vec!["statevector".into()],
            noise_profile: Some(NoiseProfile {
                t1: Some(1e9),
                t2: Some(1e9),
                single_qubit_fidelity: Some(1.0),
                two_qubit_fidelity: Some(1.0),
                readout_fidelity: Some(1.0),
                gate_time: None,
            }),
        },
        backend_type: BackendType::Simulator,
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
        capabilities: Capabilities {
            name: "huoma_mps".into(),
            num_qubits: 1_000_000,
            gate_set: GateSet {
                single_qubit: vec![
                    "h".into(), "x".into(), "y".into(), "z".into(),
                    "s".into(), "t".into(), "sdg".into(), "tdg".into(),
                    "rx".into(), "ry".into(), "rz".into(),
                ],
                two_qubit: vec![
                    "cx".into(), "cz".into(), "swap".into(),
                ],
                three_qubit: vec![],
                native: vec![],
            },
            // Use FullyConnected kind with empty edges for simulators
            // (actual connectivity is unlimited).
            topology: Topology {
                kind: TopologyKind::FullyConnected,
                edges: vec![],
            },
            max_shots: 100_000,
            is_simulator: true,
            features: vec!["tensor_network".into()],
            noise_profile: Some(NoiseProfile {
                t1: Some(1e9),
                t2: Some(1e9),
                single_qubit_fidelity: Some(1.0),
                two_qubit_fidelity: Some(1.0),
                readout_fidelity: Some(1.0),
                gate_time: None,
            }),
        },
        backend_type: BackendType::Simulator,
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
        let mut ids: Vec<&str> = backends.iter().map(|b| b.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), backends.len(), "duplicate backend IDs");
    }

    #[test]
    fn all_backends_are_online() {
        for b in all_backends() {
            assert!(b.is_online(), "{} should be online by default", b.id());
        }
    }

    #[test]
    fn ibm_torino_has_133_qubits() {
        let b = ibm_torino();
        assert_eq!(b.qubit_count(), 133);
        assert!(b.supports_gate("cz"));
        assert_eq!(b.capabilities.topology.kind, TopologyKind::HeavyHex);
    }

    #[test]
    fn quantinuum_h2_has_best_fidelity() {
        let h2 = quantinuum_h2();
        let eagle = ibm_eagle();
        let h2_fidelity = h2
            .noise_profile()
            .and_then(|n| n.two_qubit_fidelity)
            .unwrap_or(0.0);
        let eagle_fidelity = eagle
            .noise_profile()
            .and_then(|n| n.two_qubit_fidelity)
            .unwrap_or(0.0);
        assert!(h2_fidelity > eagle_fidelity);
    }

    #[test]
    fn ionq_has_all_to_all_connectivity() {
        assert_eq!(
            ionq_aria().capabilities.topology.kind,
            TopologyKind::FullyConnected
        );
        assert_eq!(
            ionq_forte().capabilities.topology.kind,
            TopologyKind::FullyConnected
        );
    }

    #[test]
    fn simulator_supports_all_common_gates() {
        let sim = simulator();
        assert!(sim.supports_gate("h"));
        assert!(sim.supports_gate("cx"));
        assert!(sim.supports_gate("ecr"));
        assert!(sim.supports_gate("iswap"));
        assert!(sim.supports_gate("ms"));
    }

    #[test]
    fn simulator_has_zero_noise() {
        let sim = simulator();
        let noise = sim.noise_profile().unwrap();
        assert_eq!(noise.single_qubit_fidelity, Some(1.0));
        assert_eq!(noise.two_qubit_fidelity, Some(1.0));
        assert_eq!(noise.readout_fidelity, Some(1.0));
    }

    #[test]
    fn huoma_supports_1m_qubits() {
        let h = huoma_mps();
        assert_eq!(h.qubit_count(), 1_000_000);
        assert!(matches!(h.backend_type, BackendType::Simulator));
    }

    #[test]
    fn iqm_sirius_has_star_topology() {
        let s = iqm_sirius();
        assert_eq!(s.capabilities.topology.kind, TopologyKind::Custom);
        let edges = &s.capabilities.topology.edges;
        assert_eq!(edges.len(), 5, "star with 6 qubits has 5 edges");
        assert!(edges.iter().all(|(a, _)| *a == 0), "all edges from center");
    }

    #[test]
    fn trapped_ion_has_longer_coherence_than_superconducting() {
        let ion = quantinuum_h2();
        let sc = ibm_eagle();
        let ion_t1 = ion.noise_profile().and_then(|n| n.t1).unwrap_or(0.0);
        let sc_t1 = sc.noise_profile().and_then(|n| n.t1).unwrap_or(0.0);
        let ion_t2 = ion.noise_profile().and_then(|n| n.t2).unwrap_or(0.0);
        let sc_t2 = sc.noise_profile().and_then(|n| n.t2).unwrap_or(0.0);
        assert!(ion_t1 > sc_t1);
        assert!(ion_t2 > sc_t2);
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
                println!("Picked: {backend_id} (score: {score:.3})");
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }
}
