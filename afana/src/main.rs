// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Afana CLI — compile Ehrenfest programs to OpenQASM.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use afana::emit::{self, QasmVersion};
use afana::optimize;
use afana::parser;

#[derive(Parser)]
#[command(name = "afana", version, about = "Ehrenfest → OpenQASM compiler")]
struct Cli {
    /// Input .ef file (text or CBOR binary).
    input: PathBuf,

    /// QASM version to emit.
    #[arg(long, default_value = "v2")]
    qasm: QasmVersionArg,

    /// Run T-gate reduction pass.
    #[arg(long)]
    reduce_t: bool,

    /// Run ZX-calculus optimization.
    #[arg(long)]
    optimize: bool,

    /// Print compilation statistics to stderr.
    #[arg(long)]
    stats: bool,
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

    // Parse the input file.
    let ast = parser::parse_file(&cli.input).context("parse failed")?;

    // Emit QASM.
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
        eprintln!("  Gates before optimization: {}", stats.gate_count_before);
        eprintln!("  Gates after optimization:  {}", stats.gate_count_after);
        if let (Some(tb), Some(ta)) = (stats.t_before, stats.t_after) {
            eprintln!("  T-gates before: {tb}");
            eprintln!("  T-gates after:  {ta}");
        }
    }

    Ok(())
}
