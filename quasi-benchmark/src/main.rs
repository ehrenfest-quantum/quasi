// SPDX-License-Identifier: AGPL-3.0-or-later
//! CLI for the GPCR quantum chemistry benchmark.
//!
//! Usage:
//!   quasi-benchmark gpcr reproduce   — match IQM QExa20 result
//!   quasi-benchmark gpcr analyse     — sin(C/2) commensurability analysis
//!   quasi-benchmark gpcr extend      — extend active space beyond QPU limit
//!   quasi-benchmark gpcr full        — full benchmark suite with report

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "quasi-benchmark")]
#[command(about = "GPCR quantum chemistry benchmark: UCL/IQM/NVIDIA vs QUASI/Huoma")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// GPCR benchmark suite
    Gpcr {
        #[command(subcommand)]
        action: GpcrAction,
    },
}

#[derive(Subcommand)]
enum GpcrAction {
    /// Reproduce the IQM QExa20 result using Huoma exact diagonalisation
    Reproduce,
    /// Run sin(C/2) commensurability analysis on the Zundel Hamiltonian
    Analyse,
    /// Extend active space beyond QPU qubit limit
    Extend,
    /// Run full benchmark suite and generate comparison report
    Full,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Gpcr { action } => match action {
            GpcrAction::Reproduce => {
                quasi_benchmark::gpcr::reproduce::reproduce_iqm_result()?;
            }
            GpcrAction::Analyse => {
                quasi_benchmark::gpcr::commensurability::analyse_zundel()?;
            }
            GpcrAction::Extend => {
                quasi_benchmark::gpcr::extend::extend_active_space()?;
            }
            GpcrAction::Full => {
                println!("=== GPCR Quantum Chemistry Benchmark ===\n");

                println!("--- Step 1: Reproduce IQM QExa20 ---");
                let result = quasi_benchmark::gpcr::reproduce::reproduce_iqm_result()?;

                println!("\n--- Step 2: Commensurability Analysis ---");
                let analysis = quasi_benchmark::gpcr::commensurability::analyse_zundel()?;

                println!("\n--- Step 3: Active Space Extension ---");
                let extension = quasi_benchmark::gpcr::extend::extend_active_space()?;

                println!("\n--- Comparison Report ---");
                quasi_benchmark::report::print_comparison(&result, &analysis, &extension);
            }
        },
    }

    Ok(())
}
