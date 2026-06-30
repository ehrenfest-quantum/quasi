// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Solvayeur Kernel Demo: Phases A–F on real IBM QPU.
//!
//! Demonstrates the full kernel stack:
//!   A) Backend selection via ATW measurement
//!   B) Qubit region allocation via calibration scoring
//!   C) Multi-program spatial scheduling
//!   D) Coherence budgeting
//!   E) Entanglement tracking
//!   F) Error budget evaluation
//!
//! The scheduling circuit runs on IBM Strasbourg (or mock).

use anyhow::Result;
use clap::Parser;

use quasi_scheduler::profiles;
use quasi_solvayeur::atw::Solvayeur;
use quasi_solvayeur::bias::Reward;
use quasi_solvayeur::calibration::BackendCalibration;
use quasi_solvayeur::coherence::{analyze_coherence, GateTiming};
use quasi_solvayeur::entanglement;
use quasi_solvayeur::error_budget::{estimate_error, evaluate, max_feasible_depth, ErrorPolicy};
use quasi_solvayeur::epoch::QpuConfig;
use quasi_solvayeur::partition::{partition_qpu, ProgramRequest};
use quasi_solvayeur::qubit_map::find_best_region;

#[derive(Parser)]
#[command(name = "kernel-demo", version, about = "Solvayeur kernel A–F demo")]
struct Cli {
    /// ATW rounds for backend selection.
    #[arg(long, default_value = "5")]
    rounds: usize,

    /// Use real QPU for ATW scheduling circuit.
    #[arg(long)]
    real_qpu: bool,

    /// QPU submission script path (for real QPU mode).
    #[arg(long, default_value = "/home/vops/solvayeur-qpu.py")]
    qpu_script: String,

    /// IBM backend for ATW circuit.
    #[arg(long, default_value = "ibm_strasbourg")]
    qpu_backend: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("================================================================");
    println!("  Solvayeur Kernel Demo: Phases A through F");
    println!("================================================================");

    // ── Setup ────────────────────────────────────────────────────────────
    let backends = profiles::all_backends();
    println!("\n[SETUP] {} backends loaded from HAL Contract profiles", backends.len());

    // Pick IBM Strasbourg for the demo (127q Eagle)
    let target = backends.iter().find(|b| b.id() == "ibm_eagle").unwrap();
    let n_qubits = target.qubit_count();
    println!("[SETUP] Target backend: {} ({}q)", target.id(), n_qubits);

    // Generate mock calibration (in production: fetch from HAL Contract)
    let edges: Vec<(u32, u32)> = target.capabilities.topology.edges
        .iter()
        .map(|&(a, b)| (a, b))
        .collect();
    let calibration = BackendCalibration::mock(target.id(), n_qubits, &edges);
    println!("[SETUP] Calibration loaded: {} qubits, {} edges", calibration.qubits.len(), calibration.edges.len());

    // ── Phase A: Backend Selection ───────────────────────────────────────
    println!("\n--- Phase A: Backend Selection (ATW) ---");

    let qpu_config = if cli.real_qpu {
        QpuConfig {
            script_path: Some(cli.qpu_script.clone()),
            shots: 100,
            backend: cli.qpu_backend.clone(),
        }
    } else {
        QpuConfig::default()
    };

    let mut solvayeur = Solvayeur::with_qpu(&backends, qpu_config);

    for round in 0..cli.rounds {
        let decision = solvayeur.decide()?;
        let selected = &backends[decision.backend_index];

        // Simulate reward based on backend quality
        let reward = Reward {
            fidelity: selected.noise_profile()
                .and_then(|n| n.single_qubit_fidelity)
                .unwrap_or(0.95),
            latency_ms: if selected.capabilities.is_simulator { 5.0 } else { 200.0 },
            cost: selected.cost_per_shot,
        };

        let r = reward.score();
        solvayeur.observe(&decision, reward);

        let qpu_marker = if decision.ran_on_qpu { " [QPU]" } else { "" };
        println!(
            "  Round {}: {} (reward {:.3}, {} gates){}",
            round, selected.id(), r, decision.gate_count, qpu_marker
        );
    }

    if let Some(preferred) = solvayeur.preferred_backend() {
        println!("  -> Preferred: {}", preferred);
    }

    // ── Phase B: Qubit Region Allocation ─────────────────────────────────
    println!("\n--- Phase B: Qubit Region Allocation ---");

    let program_qubits = 20;
    let region = find_best_region(program_qubits, &calibration, &edges, n_qubits);
    println!("  Program needs {} qubits", program_qubits);
    println!("  Best region: {:?}", &region.mapping[..region.mapping.len().min(10)]);
    if region.mapping.len() > 10 {
        println!("    ... ({} total)", region.mapping.len());
    }
    println!("  Region score: {:.4}", region.score);

    // Show worst qubit the kernel avoided
    let worst_qubit = (0..n_qubits)
        .min_by(|&a, &b| calibration.qubit_score(a).partial_cmp(&calibration.qubit_score(b)).unwrap())
        .unwrap();
    let avoided = !region.mapping.contains(&worst_qubit);
    println!(
        "  Worst qubit on chip: q{} (score {:.4}) — {}",
        worst_qubit,
        calibration.qubit_score(worst_qubit),
        if avoided { "AVOIDED" } else { "included (no better option)" }
    );

    // ── Phase C: Multi-Program Scheduling ────────────────────────────────
    println!("\n--- Phase C: Multi-Program Spatial Scheduling ---");

    let programs = vec![
        ProgramRequest { id: "vqe_h2".into(), n_qubits: 4, priority: 10 },
        ProgramRequest { id: "qaoa_maxcut".into(), n_qubits: 8, priority: 5 },
        ProgramRequest { id: "grover_search".into(), n_qubits: 6, priority: 3 },
        ProgramRequest { id: "hubbard_sim".into(), n_qubits: 20, priority: 8 },
        ProgramRequest { id: "ising_chain".into(), n_qubits: 50, priority: 2 },
    ];

    let plan = partition_qpu(&programs, &calibration, &edges, n_qubits);

    println!("  {} programs submitted to {}q backend:", programs.len(), n_qubits);
    for alloc in &plan.allocations {
        let qubits_preview: Vec<String> = alloc.physical_qubits.iter().take(5).map(|q| format!("q{}", q)).collect();
        let suffix = if alloc.physical_qubits.len() > 5 { format!("... ({})", alloc.physical_qubits.len()) } else { String::new() };
        println!(
            "    {} ({}q, pri={}) -> [{}{}] score={:.3}",
            alloc.program_id,
            alloc.physical_qubits.len(),
            programs.iter().find(|p| p.id == alloc.program_id).map(|p| p.priority).unwrap_or(0),
            qubits_preview.join(", "),
            suffix,
            alloc.region_score,
        );
    }
    for rej in &plan.rejected {
        println!("    {} -> REJECTED (insufficient qubits)", rej);
    }
    println!(
        "  Utilization: {:.0}% ({} of {} qubits), {} idle",
        plan.utilization * 100.0,
        plan.total_qubits - plan.idle_qubits,
        plan.total_qubits,
        plan.idle_qubits,
    );

    // ── Phase D: Coherence Budgeting ─────────────────────────────────────
    println!("\n--- Phase D: Coherence Budgeting ---");

    let timing = GateTiming::default();

    // Check each allocated program against T2
    let demo_circuits = [
        ("vqe_h2", 10u32, 20u32, 5u32, 4u32),       // depth, 1q, 2q, meas
        ("qaoa_maxcut", 30, 60, 20, 8),
        ("grover_search", 50, 100, 30, 6),
        ("hubbard_sim", 200, 400, 100, 20),
        ("ising_chain", 500, 1000, 250, 50),
    ];

    for (prog_id, _depth, n_1q, n_2q, n_meas) in &demo_circuits {
        if let Some(alloc) = plan.allocations.iter().find(|a| a.program_id == *prog_id) {
            let qubit_t2s: Vec<(u32, f64)> = alloc.physical_qubits.iter()
                .map(|&q| (q, calibration.qubits[q as usize].t2))
                .collect();

            let analysis = analyze_coherence(*_depth, *n_1q, *n_2q, *n_meas, &qubit_t2s, &timing);

            let status = match &analysis.recommendation {
                quasi_solvayeur::coherence::CoherenceAction::Proceed => "OK".to_string(),
                quasi_solvayeur::coherence::CoherenceAction::Warn { t2_ratio } =>
                    format!("WARN (T2 ratio {:.2})", t2_ratio),
                quasi_solvayeur::coherence::CoherenceAction::Exceed { suggest_split_count, .. } =>
                    format!("EXCEED -> split into {} sub-circuits", suggest_split_count),
            };

            println!(
                "  {}: {:.1}μs circuit, T2_min={:.0}μs -> {}",
                prog_id, analysis.circuit_time_us, analysis.min_t2_us, status,
            );
        }
    }

    // ── Phase E: Entanglement Tracking ───────────────────────────────────
    println!("\n--- Phase E: Entanglement Tracking ---");

    // Simulate gate sequences for two programs that share entanglement
    let mut emap = entanglement::EntanglementMap::new();

    // VQE creates entanglement between qubits 0-1 and 2-3
    let vqe_gates: Vec<(String, u32, u32)> = vec![
        ("cx".into(), 0, 1),
        ("cx".into(), 2, 3),
    ];
    for (gate, a, b) in &vqe_gates {
        emap.add_link(*a, *b, "vqe_h2", gate.as_str());
    }

    // QAOA creates entanglement on different qubits
    let qaoa_gates: Vec<(String, u32, u32)> = vec![
        ("cz".into(), 4, 5),
        ("cz".into(), 5, 6),
        ("cz".into(), 6, 7),
    ];
    for (gate, a, b) in &qaoa_gates {
        emap.add_link(*a, *b, "qaoa_maxcut", gate.as_str());
    }

    println!("  Active entanglement links: {}", emap.active_link_count());
    println!(
        "  VQE qubits entangled: {:?}",
        emap.entangled_with(0)
    );
    println!(
        "  QAOA qubits entangled: {:?}",
        emap.entangled_with(4)
    );

    let independent = entanglement::can_schedule_independently(&emap, "vqe_h2", "qaoa_maxcut");
    println!(
        "  VQE and QAOA independent? {} -> {}",
        independent,
        if independent { "can run in parallel" } else { "must sequence" }
    );

    // Consume entanglement via measurement
    emap.consume(1);
    println!("  After measuring q1: {} active links", emap.active_link_count());

    // ── Phase F: Error Budget ────────────────────────────────────────────
    println!("\n--- Phase F: Error Budget ---");

    let policy = ErrorPolicy::default();

    for (prog_id, _depth, n_1q, n_2q, n_meas) in &demo_circuits {
        let budget = estimate_error(
            *n_1q, *n_2q, *n_meas,
            0.999,  // avg 1Q fidelity
            0.99,   // avg 2Q fidelity
            0.98,   // avg readout fidelity
        );

        let verdict = evaluate(&budget, &policy);

        let status = match &verdict {
            quasi_solvayeur::error_budget::ErrorVerdict::Ok { total_error } =>
                format!("OK ({:.1}% error)", total_error * 100.0),
            quasi_solvayeur::error_budget::ErrorVerdict::Warn { total_error, .. } =>
                format!("WARN ({:.1}% error)", total_error * 100.0),
            quasi_solvayeur::error_budget::ErrorVerdict::Halt { total_error, suggestion, .. } =>
                format!("HALT ({:.1}% error) -> {:?}", total_error * 100.0, suggestion),
        };

        println!("  {}: {} gates -> {}", prog_id, n_1q + n_2q, status);
    }

    // What's the max depth we can run on this hardware?
    let max_depth = max_feasible_depth(0.90, 0.999, 0.99, 0.3);
    println!("\n  Max feasible depth (90% fidelity target): {} gates", max_depth);

    // ── Summary ──────────────────────────────────────────────────────────
    println!("\n================================================================");
    println!("  Solvayeur Kernel: 6 phases, 1 dispatch cycle");
    println!("================================================================");
    println!("  A) Backend:      {} selected via ATW", solvayeur.preferred_backend().unwrap_or("?"));
    println!("  B) Qubits:       {} physical qubits mapped (score {:.3})", region.mapping.len(), region.score);
    println!("  C) Programs:     {} running, {} rejected, {:.0}% utilization",
        plan.allocations.len(), plan.rejected.len(), plan.utilization * 100.0);
    println!("  D) Coherence:    T2 checked per program per qubit");
    println!("  E) Entanglement: {} links tracked, dependencies resolved", emap.active_link_count());
    println!("  F) Error budget: warn at 5%, halt at 10%, max depth {} gates", max_depth);
    if cli.real_qpu {
        println!("\n  ATW scheduling circuit ran on REAL QPU ({}).", cli.qpu_backend);
    }
    println!("================================================================");

    Ok(())
}
