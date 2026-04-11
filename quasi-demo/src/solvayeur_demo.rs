// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Solvayeur Demo: The ATW kernel dispatching workloads across mock backends.
//!
//! Shows the Solvayeur quantum kernel learning which backend is best by
//! running ATW rounds — each round compiles a scheduling Hamiltonian through
//! Afana, measures, dispatches, observes reward, and updates the bias.
//!
//! Usage:
//!   solvayeur-demo                     # 40 rounds, 4 mock backends
//!   solvayeur-demo --rounds 100        # more rounds, watch convergence
//!   solvayeur-demo --backends 8        # 8 backends = 3-qubit scheduling circuit

use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;

use quasi_scheduler::backend::*;
use quasi_solvayeur::atw::Solvayeur;
use quasi_solvayeur::bias::Reward;

#[derive(Parser)]
#[command(name = "solvayeur-demo", version, about = "Solvayeur ATW kernel demo")]
struct Cli {
    /// Number of ATW rounds.
    #[arg(long, default_value = "40")]
    rounds: usize,

    /// Number of mock backends (2, 4, or 8).
    #[arg(long, default_value = "4")]
    backends: usize,
}

// ── Mock Backend Simulation ──────────────────────────────────────────────────

/// A mock backend that returns results with characteristic fidelity and latency.
struct MockBackend {
    name: String,
    fidelity: f64,    // base fidelity (0 to 1)
    latency_ms: f64,  // base latency
    cost: f64,        // cost per shot
    /// Simulate variance — backends aren't perfectly consistent.
    noise: f64,
}

impl MockBackend {
    /// Simulate executing a workload. Returns (fidelity, latency, cost).
    fn execute(&self, round: usize) -> (f64, f64, f64) {
        // Add deterministic "noise" based on round number
        let jitter = ((round as f64 * 7.3 + self.cost * 1000.0).sin() * self.noise).abs();
        let fidelity = (self.fidelity - jitter).clamp(0.0, 1.0);
        let latency = self.latency_ms * (1.0 + jitter);
        (fidelity, latency, self.cost)
    }
}

fn create_mock_backends(n: usize) -> (Vec<Backend>, Vec<MockBackend>) {
    let specs: Vec<(&str, f64, f64, f64, f64)> = vec![
        // (name, fidelity, latency_ms, cost, noise)
        // Deliberately competitive: no single backend dominates on all axes.
        // Huoma is fast+cheap but "only" 0.95 fidelity (truncation error at depth).
        // Quantinuum is highest fidelity but very expensive.
        // IBM is the balanced choice.
        ("huoma_mps",       0.95,  5.0,    0.001, 0.03),   // Fast, cheap, good but not perfect
        ("ibm_strasbourg",  0.92,  200.0,  0.01,  0.05),   // Balanced: decent everything
        ("iqm_garnet",      0.85,  150.0,  0.005, 0.08),   // Cheaper but noisier
        ("quantinuum_h2",   0.98,  500.0,  0.08,  0.01),   // Best fidelity, expensive
        ("ibm_marrakesh",   0.90,  180.0,  0.01,  0.06),   // Similar to Strasbourg
        ("rigetti_ankaa3",  0.82,  120.0,  0.007, 0.10),   // Fastest QPU, noisiest
        ("ionq_forte",      0.96,  400.0,  0.05,  0.02),   // High fidelity, medium cost
        ("simulator",       1.00,  2.0,    0.0001,0.0),     // Perfect but trivial
    ];

    let count = n.min(specs.len());

    let scheduler_backends: Vec<Backend> = specs[..count]
        .iter()
        .map(|(name, _, _, cost, _)| Backend {
            id: name.to_string(),
            name: name.to_string(),
            backend_type: if name.contains("sim") || name.contains("huoma") {
                BackendType::Simulator
            } else {
                BackendType::Qpu
            },
            qubit_count: 100,
            gate_set: GateSet {
                native_gates: vec!["h".into(), "cx".into(), "rz".into()],
            },
            topology: Topology::AllToAll,
            noise: NoiseProfile {
                t1_us: 200.0,
                t2_us: 100.0,
                single_qubit_error: 0.001,
                two_qubit_error: 0.01,
                readout_error: 0.02,
                calibration_version: "mock".into(),
            },
            status: BackendStatus::Online { queue_depth: 0 },
            cost_per_shot: *cost,
        })
        .collect();

    let mock_backends: Vec<MockBackend> = specs[..count]
        .iter()
        .map(|(name, fid, lat, cost, noise)| MockBackend {
            name: name.to_string(),
            fidelity: *fid,
            latency_ms: *lat,
            cost: *cost,
            noise: *noise,
        })
        .collect();

    (scheduler_backends, mock_backends)
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (backends, mock_backends) = create_mock_backends(cli.backends);
    let n_qubits = (backends.len() as f64).log2().ceil() as usize;

    println!("================================================================");
    println!("  Solvayeur Demo: ATW Quantum Kernel");
    println!("================================================================");
    println!();
    println!("  Algorithm:  ATW (Around The World)");
    println!("  Backends:   {} (encoded in {} qubits)", backends.len(), n_qubits);
    println!("  Rounds:     {}", cli.rounds);
    println!("  Learning:   eta=0.3, lambda=0.1, Gamma=1.0 (annealing)");
    println!();
    println!("  Mock backend characteristics:");
    for mb in &mock_backends {
        let star = if mb.fidelity > 0.95 && mb.latency_ms < 100.0 && mb.cost < 0.01 {
            " <-- optimal"
        } else {
            ""
        };
        println!(
            "    {:20} fidelity={:.3}  latency={:>7.0}ms  cost={:.4}{}",
            mb.name, mb.fidelity, mb.latency_ms, mb.cost, star
        );
    }
    println!();
    println!("----------------------------------------------------------------");
    println!(
        "  {:>5} | {:20} | {:>7} | {:>6} | bias",
        "round", "dispatched to", "reward", "Gamma"
    );
    println!("----------------------------------------------------------------");

    let mut solvayeur = Solvayeur::new(&backends);

    let mut selection_counts: HashMap<usize, usize> = HashMap::new();

    for round in 0..cli.rounds {
        // 1. ATW decides: compile H(k) through Afana → measure → backend index
        let decision = solvayeur.decide()?;

        // 2. Mock backend executes the "workload"
        let backend_idx = decision.backend_index;
        let (fidelity, latency, cost) = mock_backends[backend_idx].execute(round);

        // 3. Observe reward and update bias
        let reward = Reward {
            fidelity,
            latency_ms: latency,
            cost,
        };
        let r = reward.score();

        solvayeur.observe(&decision, reward);

        *selection_counts.entry(backend_idx).or_insert(0) += 1;

        // 4. Print
        let bias_str: String = solvayeur
            .bias()
            .iter()
            .map(|b| format!("{:+.2}", b))
            .collect::<Vec<_>>()
            .join(" ");

        let backend_name = &mock_backends[backend_idx].name;
        let marker = if r > 0.8 {
            "+"
        } else if r > 0.5 {
            "~"
        } else {
            "-"
        };

        println!(
            "  {:>5} | {:20} | {:>6.3} {} | {:>5.2} | [{}]",
            round,
            backend_name,
            r,
            marker,
            solvayeur.exploration(),
            bias_str
        );
    }

    println!("----------------------------------------------------------------");
    println!();

    // ── Summary ──────────────────────────────────────────────────────────────

    println!("================================================================");
    println!("  ATW Convergence Report");
    println!("================================================================");
    println!();

    println!("  Selection frequency:");
    let mut sorted_counts: Vec<_> = selection_counts.iter().collect();
    sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
    for (idx, count) in &sorted_counts {
        let pct = **count as f64 / cli.rounds as f64 * 100.0;
        let bar_len = (pct / 2.0) as usize;
        let bar: String = "#".repeat(bar_len);
        println!(
            "    {:20} {:>4} ({:>5.1}%) {}",
            mock_backends[**idx].name, count, pct, bar
        );
    }

    println!();
    println!("  Final bias fields: {:?}", solvayeur.bias());
    println!("  Final exploration:  {:.4}", solvayeur.exploration());
    if let Some(preferred) = solvayeur.preferred_backend() {
        println!("  Preferred backend:  {}", preferred);
    }

    // Show that the scheduling circuit went through Afana
    let final_decision = solvayeur.decide()?;
    println!();
    println!("  Last scheduling circuit:");
    println!("    Gates:  {}", final_decision.gate_count);
    println!("    QASM preview:");
    for line in final_decision.qasm.lines().take(8) {
        println!("      {}", line);
    }
    println!("      ...");

    println!();
    println!("================================================================");
    println!("  The Solvayeur kernel compiled {} scheduling Hamiltonians", cli.rounds);
    println!("  through Afana. Each measurement was a dispatching decision.");
    println!("  The OS compiled itself. Around the world.");
    println!("================================================================");

    Ok(())
}
