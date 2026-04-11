// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! End-to-end QUASI demo: compile an Ehrenfest program, profile the workload,
//! check the cache, optionally submit to a QPU via the HAL Contract API,
//! and display results.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use afana::cbor;
use afana::emit::{self, QasmVersion};
use afana::lower;
use afana::noise;
use afana::trotter::{self, TrotterOrder};
use afana::type_check;
use quasi_cache::store::{CacheConfig, CacheStore};
use quasi_cache::CacheEntry;
use quasi_cache::CacheKey;
use quasi_scheduler::profiler::{HeuristicProfiler, WorkloadProfiler};

#[derive(Parser)]
#[command(
    name = "quasi-demo",
    version,
    about = "End-to-end QUASI demo: compile, profile, cache, and run on QPU"
)]
struct Cli {
    /// Input Ehrenfest program (hex-encoded CBOR file).
    input: PathBuf,

    /// QPU backend name.
    #[arg(long, default_value = "ibm_torino")]
    backend: String,

    /// Number of shots.
    #[arg(long, default_value_t = 4096)]
    shots: u32,

    /// HAL Contract base URL.
    #[arg(long, default_value = "https://gawain.valiant-quantum.com")]
    hal_url: String,

    /// Cache directory for L2 persistence.
    #[arg(long, default_value = "/tmp/quasi-cache")]
    cache_dir: PathBuf,

    /// Skip QPU submission, only compile and profile.
    #[arg(long)]
    skip_qpu: bool,

    /// Trotter decomposition order (1 or 2).
    #[arg(long, default_value_t = 2)]
    trotter_order: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // -- Step 1: Read CBOR hex file and decode --
    println!("===================================================");
    println!("  QUASI Demo: End-to-End Quantum Compilation Pipeline");
    println!("===================================================");
    println!();

    let hex_str = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("failed to read {}", cli.input.display()))?
        .trim()
        .to_string();

    let cbor_bytes: Vec<u8> = (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16))
        .collect::<Result<_, _>>()
        .context("invalid hex encoding in input file")?;

    let program = cbor::from_cbor(&cbor_bytes).context("CBOR deserialization failed")?;

    println!("[LOAD] Program loaded");
    println!("   Qubits:       {}", program.system.n_qubits);
    println!(
        "   Hamiltonian:   {} terms",
        program.hamiltonian.terms.len()
    );
    println!(
        "   Evolution:     {} us ({} steps, dt={} us)",
        program.evolution.total_us, program.evolution.steps, program.evolution.dt_us
    );
    println!("   Observables:   {}", program.observables.len());
    println!();

    // -- Step 2: Compile with Afana --
    println!("[COMPILE] Compiling with Afana...");
    let order = match cli.trotter_order {
        2 => TrotterOrder::Second,
        _ => TrotterOrder::First,
    };
    let ast = trotter::trotterize(&program, order);

    // Type check.
    type_check::type_check_ast(&ast).map_err(|errs| {
        let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("Type check failed: {}", msgs.join(", "))
    })?;

    // Lower to ZX-IR.
    let zx = lower::lower_ast_to_zx(&ast).context("ZX-IR lowering failed")?;
    zx.validate().map_err(|errs| {
        let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("ZX-IR validation failed: {}", msgs.join(", "))
    })?;

    // Noise analysis.
    let noise_report = noise::analyze_noise(&ast, &program.noise, &program.evolution);

    // Emit QASM 3.0.
    let qasm = emit::emit_qasm(&ast, QasmVersion::V3).context("QASM emission failed")?;

    println!("   Gates:         {}", ast.gates.len());
    println!("   Circuit depth: {}", noise_report.circuit_depth);
    println!(
        "   ZX-IR:         {} spiders, {} edges",
        zx.spider_count(),
        zx.edge_count()
    );
    println!(
        "   Est. fidelity: {:.3}",
        noise_report.estimated_fidelity
    );
    if !noise_report.warnings.is_empty() {
        println!("   Warnings:      {}", noise_report.warnings.len());
    }
    println!();

    // -- Step 3: Workload profiling --
    println!("[PROFILE] Profiling workload...");
    let profiler = HeuristicProfiler::default();
    let profile = profiler.profile(
        &[0u8; 32],
        noise_report.circuit_depth as u32,
        program.system.n_qubits as u32,
    );
    println!("   Bond dimension:  {}", profile.peak_bond_dimension);
    println!("   Entanglement:    {:?}", profile.entanglement_class);
    println!(
        "   Classical ok:    {}",
        profile.is_classically_tractable()
    );
    println!("   Routing conf.:   {:.2}", profile.routing_confidence());
    println!();

    // -- Step 4: Cache check --
    let cache = CacheStore::new(CacheConfig {
        max_l1_entries: 1000,
        l2_dir: Some(cli.cache_dir.clone()),
        max_age_seconds: 14400,
    });
    let cache_key = CacheKey::build(
        &cbor_bytes,
        &cli.backend,
        &BTreeMap::new(),
        "default",
    );
    if let Some(cached) = cache.get(&cache_key) {
        println!("[CACHE] Cache HIT -- returning stored result");
        println!("   Backend:    {}", cached.backend_id);
        println!("   Shots:      {}", cached.shot_count);
        println!("   Timestamp:  {}", cached.timestamp);
        print_counts(&cached.measurement_counts);
        return Ok(());
    }
    println!("[CACHE] Cache MISS -- will execute");
    println!();

    // -- Step 5: Submit to QPU via HAL Contract --
    if !cli.skip_qpu {
        println!(
            "[QPU] Submitting to {} via HAL Contract...",
            cli.backend
        );
        println!("   URL: {}/hal/jobs", cli.hal_url);
        println!("   Shots: {}", cli.shots);

        let client = reqwest::Client::new();

        let submit_body = serde_json::json!({
            "qasm": qasm,
            "backend": cli.backend,
            "shots": cli.shots,
        });
        let submit_resp = client
            .post(format!("{}/hal/jobs", cli.hal_url))
            .json(&submit_body)
            .send()
            .await
            .context("HAL submit request failed")?;

        if !submit_resp.status().is_success() {
            let status = submit_resp.status();
            let body = submit_resp.text().await.unwrap_or_default();
            anyhow::bail!("HAL submit failed ({}): {}", status, body);
        }

        let job: serde_json::Value = submit_resp
            .json()
            .await
            .context("failed to parse HAL submit response")?;
        let job_id = job["jobId"].as_str().unwrap_or("unknown");
        println!("   Job ID: {}", job_id);

        // Poll for result.
        println!("   Waiting for result...");
        let mut attempts = 0u32;
        let result = loop {
            attempts += 1;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let poll_resp = client
                .get(format!("{}/hal/jobs/{}", cli.hal_url, job_id))
                .send()
                .await
                .context("HAL poll request failed")?;

            if !poll_resp.status().is_success() {
                if attempts > 150 {
                    anyhow::bail!("Job polling failed after 5 minutes");
                }
                continue;
            }

            let result: serde_json::Value = poll_resp
                .json()
                .await
                .context("failed to parse HAL poll response")?;
            let status = result["status"].as_str().unwrap_or("unknown");

            match status {
                "done" => break result,
                "failed" => anyhow::bail!("Job failed: {:?}", result),
                _ => {
                    if attempts > 150 {
                        anyhow::bail!("Job timed out after 5 minutes");
                    }
                    if attempts.is_multiple_of(5) {
                        println!("   ... still running ({} polls)", attempts);
                    }
                }
            }
        };

        println!();
        println!("[RESULT] QPU Result ({}):", cli.backend);
        if let Some(counts) = result["counts"].as_object() {
            let counts_map: HashMap<String, u64> = counts
                .iter()
                .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                .collect();
            print_counts(&counts_map);

            // Cache the result.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let entry = CacheEntry {
                key: cache_key,
                backend_id: cli.backend.clone(),
                calibration_version: "default".into(),
                parameters: BTreeMap::new(),
                measurement_counts: counts_map,
                expectation_values: None,
                fidelity_estimate: Some(noise_report.estimated_fidelity),
                shot_count: cli.shots,
                timestamp: now,
                attestation_ref: None,
            };
            cache.put(entry);
            println!("   Result cached (key: {})", cache_key);
        }
    } else {
        println!("[QPU] Submission skipped (--skip-qpu)");
    }

    println!();
    println!("===================================================");
    println!("  Pipeline complete.");
    println!("===================================================");

    Ok(())
}

fn print_counts(counts: &HashMap<String, u64>) {
    let total: u64 = counts.values().sum();
    if total == 0 {
        println!("   (no counts)");
        return;
    }
    let mut sorted: Vec<_> = counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (bitstring, count) in sorted.iter().take(10) {
        let pct = **count as f64 / total as f64 * 100.0;
        println!("   |{}> {:>6} ({:.1}%)", bitstring, count, pct);
    }
    if sorted.len() > 10 {
        println!("   ... and {} more states", sorted.len() - 10);
    }
}
