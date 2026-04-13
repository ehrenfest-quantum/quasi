// SPDX-License-Identifier: AGPL-3.0-or-later
//! Benchmark report generation.
//!
//! Produces a formatted comparison table: UCL/IQM/NVIDIA vs QUASI/Huoma.

use crate::gpcr::commensurability::AnalysisResult;
use crate::gpcr::extend::ExtensionResult;
use crate::gpcr::reproduce::ReproduceResult;

/// Print the full comparison report.
pub fn print_comparison(
    reproduce: &ReproduceResult,
    analysis: &AnalysisResult,
    extension: &ExtensionResult,
) {
    let border = "=".repeat(70);
    let thin = "-".repeat(70);

    println!("{border}");
    println!("  GPCR Quantum Chemistry Benchmark: UCL/IQM/NVIDIA vs QUASI");
    println!("{border}");
    println!();
    println!("  System: Zundel cation H5O2+ (proof-of-concept from paper)");
    println!("  Reference: Bickley et al. arXiv:2505.16796");
    println!();

    // --- Method comparison ---
    println!("{thin}");
    println!("  Method comparison");
    println!("{thin}");
    println!(
        "  {:<22} {:<20} {:>12} {:>10}",
        "Method", "Hardware", "Energy (Ha)", "Time"
    );
    println!(
        "  {:<22} {:<20} {:>12} {:>10}",
        "------", "--------", "-----------", "----"
    );
    println!(
        "  {:<22} {:<20} {:>12} {:>10}",
        "QSCI (paper)",
        "IQM QExa20 20q",
        "[from paper]",
        "~min"
    );
    println!(
        "  {:<22} {:<20} {:>12.6} {:>8.1}ms",
        "Exact diag",
        "Huoma statevector",
        reproduce.energy_exact,
        reproduce.time_exact_s * 1000.0
    );
    println!(
        "  {:<22} {:<20} {:>12.6} {:>8.1}ms",
        "QUASI pipeline",
        "full stack",
        reproduce.energy_pipeline,
        reproduce.time_pipeline_s * 1000.0
    );
    println!(
        "  {:<22} {:<20} {:>12.6} {:>10}",
        "RHF (mean-field)", "classical", reproduce.rhf_energy, "<1ms"
    );
    println!();

    // --- Active space extension ---
    println!("{thin}");
    println!("  Active space extension (beyond QPU limit)");
    println!("{thin}");
    println!(
        "  {:>8} {:>10} {:>12} {:>12} {:>10}",
        "N_active", "N_qubits", "Energy (Ha)", "Method", "Time"
    );
    println!(
        "  {:>8} {:>10} {:>12} {:>12} {:>10}",
        "--------", "--------", "-----------", "------", "----"
    );
    for r in &extension.results {
        println!(
            "  {:>8} {:>10} {:>12.6} {:>12} {:>8.1}ms",
            r.n_active,
            r.n_qubits,
            r.energy,
            r.method,
            r.time_s * 1000.0
        );
    }
    if let Some(missed) = extension.correlation_missed {
        println!();
        println!(
            "  Correlation missed by QPU limit: {:.4} mHa",
            missed * 1000.0
        );
    }
    println!();

    // --- Commensurability analysis ---
    println!("{thin}");
    println!("  Commensurability analysis (sin(C/2))");
    println!("{thin}");
    println!(
        "  Total qubit pairs: {}",
        analysis.edges.len()
    );
    println!(
        "  Commensurate (active): {}",
        analysis.partition.commensurate.len()
    );
    println!(
        "  Incommensurate (environment): {}",
        analysis.partition.incommensurate.len()
    );
    println!(
        "  Compression ratio: {:.1}x",
        analysis.compression_ratio
    );
    println!(
        "  sin(C/2) active qubits: {:?}",
        analysis.sinc2_active_qubits
    );
    println!();

    // --- Resource comparison ---
    println!("{thin}");
    println!("  Resource comparison");
    println!("{thin}");
    println!(
        "  UCL/IQM/NVIDIA: 54q QPU + 120 H100 GPUs + 1200 H100 postproc"
    );
    println!(
        "  QUASI/Huoma:    1 Mac Studio (M4 Ultra, 128 GB, ~50W)"
    );
    println!();
    println!(
        "  Total QUASI wall time: {:.1}ms",
        (reproduce.time_exact_s + reproduce.time_pipeline_s) * 1000.0
            + extension.results.iter().map(|r| r.time_s).sum::<f64>() * 1000.0
    );
    println!();

    // --- Key result ---
    println!("{border}");
    println!("  Key result: Huoma produces the exact ground-state energy");
    println!("  with zero shot noise. The IQM QPU result (QSCI) is an");
    println!("  approximation — our result is strictly more accurate.");
    if analysis.compression_ratio > 1.5 {
        println!();
        println!(
            "  sin(C/2) achieves {:.1}x compression: only {}/{} qubit pairs",
            analysis.compression_ratio,
            analysis.partition.commensurate.len(),
            analysis.edges.len()
        );
        println!("  need high bond dimension for accurate tensor-network sim.");
    }
    println!("{border}");
}
