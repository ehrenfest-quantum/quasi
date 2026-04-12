// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Afana CLI — compile Ehrenfest programs to OpenQASM.
//!
//! Ehrenfest programs are CBOR binary. There is no text form.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use afana::cbor;
use afana::decompose;
use afana::emit::{self, QasmVersion};
use afana::lower;
use afana::noise;
use afana::observable;
use afana::optimize;
use afana::trotter::{self, TrotterOrder};
use afana::type_check;

#[derive(Parser)]
#[command(name = "afana", version, about = "Ehrenfest → OpenQASM compiler")]
struct Cli {
    /// Input Ehrenfest program (CBOR binary).
    input: PathBuf,

    /// QASM version to emit.
    #[arg(long, default_value = "v2")]
    qasm: QasmVersionArg,

    /// Run T-gate reduction pass.
    #[arg(long)]
    reduce_t: bool,

    /// Trotter decomposition order (1 or 2).
    #[arg(long, default_value = "1")]
    trotter_order: u32,

    /// Run ZX-calculus optimization.
    #[arg(long)]
    optimize: bool,

    /// Decompose non-Clifford gates (CCX, Rz/Rx/Ry) into Clifford+T.
    #[arg(long)]
    decompose: bool,

    /// Approximation precision for rotation decomposition (radians).
    #[arg(long, default_value = "0.0001")]
    precision: f64,

    /// Print compilation statistics to stderr.
    #[arg(long)]
    stats: bool,

    /// Bind variational parameters: --param theta=1.57 --param phi=0.5
    #[arg(long = "param", value_name = "KEY=VALUE")]
    params: Vec<String>,
}

#[derive(Clone, ValueEnum)]
enum QasmVersionArg {
    V2,
    V3,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let version = match cli.qasm {
        QasmVersionArg::V2 => QasmVersion::V2,
        QasmVersionArg::V3 => QasmVersion::V3,
    };

    // Deserialize CBOR binary program.
    let program = cbor::from_cbor_file(&cli.input).context("CBOR deserialization failed")?;

    // Trotterize: Hamiltonian → gate sequence.
    let order = match cli.trotter_order {
        2 => TrotterOrder::Second,
        _ => TrotterOrder::First,
    };
    let mut ast = trotter::trotterize(&program, order);

    // Substitute variational parameters if --param flags given.
    if !cli.params.is_empty() {
        let mut bindings = std::collections::HashMap::new();
        for kv in &cli.params {
            let (key, val) = kv
                .split_once('=')
                .with_context(|| format!("invalid --param format: {kv:?} (expected KEY=VALUE)"))?;
            let val: f64 = val
                .parse()
                .with_context(|| format!("invalid float value for param {key:?}: {val:?}"))?;
            bindings.insert(key.to_string(), val);
        }
        ast = emit::substitute_params(&ast, &bindings).context("parameter substitution failed")?;
    }

    // Decompose non-Clifford gates into Clifford+T if requested.
    let decompose_stats = if cli.decompose {
        let (decomposed, stats) = decompose::decompose_non_clifford(&ast, cli.precision);
        ast = decomposed;
        Some(stats)
    } else {
        None
    };

    // Type check.
    type_check::type_check_ast(&ast)
        .map_err(|errs| {
            let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
            anyhow::anyhow!("type check failed:\n  {}", msgs.join("\n  "))
        })?;

    // Lower to ZX-IR and validate.
    let zx = lower::lower_ast_to_zx(&ast).context("ZX-IR lowering failed")?;
    zx.validate().map_err(|errs| {
        let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("ZX-IR validation failed:\n  {}", msgs.join("\n  "))
    })?;

    // Noise-aware circuit analysis.
    let noise_report = noise::analyze_noise(&ast, &program.noise, &program.evolution);

    // Observable measurement synthesis.
    let measurements =
        observable::synthesize_measurements(&program.observables, program.system.n_qubits);
    observable::apply_measurement_circuits(&mut ast, &measurements);

    // Emit QASM (still from AST — ZX→QASM extraction is a follow-up).
    let qasm = emit::emit_qasm(&ast, version).context("QASM emission failed")?;

    // Optimize if requested.
    let (output, stats) = if cli.optimize || cli.reduce_t {
        optimize::optimize_qasm(&qasm, cli.reduce_t)
    } else {
        let stats = optimize::OptStats {
            gate_count_before: optimize::count_qasm_gates(&qasm),
            gate_count_after: optimize::count_qasm_gates(&qasm),
            ..Default::default()
        };
        (qasm, stats)
    };

    // Print output.
    println!("{output}");

    // Print stats to stderr if requested.
    if cli.stats {
        eprintln!("--- Compilation stats ---");
        eprintln!("  Program: {}", ast.name);
        eprintln!("  Qubits:  {}", ast.n_qubits);
        eprintln!("  ZX-IR spiders: {}", zx.spider_count());
        eprintln!("  ZX-IR edges:   {}", zx.edge_count());
        eprintln!("  Gates before optimization: {}", stats.gate_count_before);
        eprintln!("  Gates after optimization:  {}", stats.gate_count_after);
        if let (Some(tb), Some(ta)) = (stats.t_before, stats.t_after) {
            eprintln!("  T-gates before: {tb}");
            eprintln!("  T-gates after:  {ta}");
        }
        if let Some(ref ds) = decompose_stats {
            eprintln!("--- Non-Clifford decomposition ---");
            eprintln!("  CCX decomposed:     {}", ds.ccx_decomposed);
            eprintln!("  Rz exact decomp:    {}", ds.rz_exact);
            eprintln!("  Rz approx decomp:   {}", ds.rz_approx);
            eprintln!("  T-count:            {}", ds.t_count);
        }
        eprintln!("--- Noise analysis ---");
        eprintln!("{}", noise::format_report(&noise_report));
        eprintln!("  Observables:    {}", measurements.len());
    }

    Ok(())
}
