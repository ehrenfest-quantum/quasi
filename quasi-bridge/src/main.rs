// SPDX-License-Identifier: AGPL-3.0-or-later
//! quasi-bridge CLI — molecular quantum chemistry pipeline.
//!
//! ```text
//! quasi-bridge run --smiles "O" --basis sto-3g --accuracy chemical
//! quasi-bridge analyze --smiles "[H][H]"
//! quasi-bridge serve --bind 0.0.0.0:9090
//! ```

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "quasi-bridge", about = "Garm → QUASI molecular pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the full pipeline: SMILES → energy
    Run {
        /// SMILES string of the molecule
        #[arg(long)]
        smiles: String,
        /// Basis set (default: sto-3g)
        #[arg(long, default_value = "sto-3g")]
        basis: String,
        /// Accuracy level: chemical, spectroscopic, exact
        #[arg(long, default_value = "chemical")]
        accuracy: String,
        /// Number of active orbitals (omit for full space)
        #[arg(long)]
        n_active: Option<usize>,
        /// Path to Garm analysis JSON
        #[arg(long)]
        garm_json: Option<String>,
    },
    /// Analyze only (RHF + partition, no quantum execution)
    Analyze {
        /// SMILES string of the molecule
        #[arg(long)]
        smiles: String,
        /// Basis set
        #[arg(long, default_value = "sto-3g")]
        basis: String,
    },
    /// Start HTTP server
    Serve {
        /// Bind address (e.g. 0.0.0.0:9090)
        #[arg(long, default_value = "127.0.0.1:9090")]
        bind: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            smiles,
            basis,
            accuracy,
            n_active,
            garm_json,
        } => {
            let acc = match accuracy.as_str() {
                "chemical" => quasi_bridge::ehrenfest_mol::Accuracy::Chemical,
                "spectroscopic" => quasi_bridge::ehrenfest_mol::Accuracy::Spectroscopic,
                "exact" => quasi_bridge::ehrenfest_mol::Accuracy::Exact,
                other => {
                    eprintln!("Unknown accuracy: {other}. Use: chemical, spectroscopic, exact");
                    std::process::exit(1);
                }
            };

            let garm = garm_json.and_then(|path| std::fs::read_to_string(path).ok());

            let config = quasi_bridge::pipeline::PipelineConfig {
                basis,
                accuracy: acc,
                n_active,
                garm_json: garm,
                verbose: true,
            };

            match quasi_bridge::pipeline::run_molecule(&smiles, &config) {
                Ok(result) => {
                    println!("--- Result ---");
                    println!("Molecule:       {}", result.smiles);
                    println!("Basis:          {}", result.basis);
                    println!("RHF energy:     {:.6} Ha", result.rhf_energy);
                    println!("Energy:         {:.6} Ha", result.energy);
                    println!(
                        "Correlation:    {:.6} Ha",
                        result.energy - result.rhf_energy
                    );
                    println!("Qubits:         {}", result.n_qubits);
                    println!("Pauli terms:    {}", result.n_pauli_terms);
                    println!("Backend:        {}", result.backend);
                    println!("Trotter steps:  {}", result.trotter_steps);
                    println!("Gate count:     {}", result.gate_count);
                    println!(
                        "Trotter error:  {:.4} mHa",
                        result.trotter_error_bound_mha
                    );
                }
                Err(e) => {
                    eprintln!("Pipeline error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Command::Serve { bind } => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(async {
                let app = quasi_bridge::server::app();
                let listener = tokio::net::TcpListener::bind(&bind)
                    .await
                    .unwrap_or_else(|e| panic!("failed to bind {bind}: {e}"));
                eprintln!("quasi-bridge serving on http://{bind}");
                axum::serve(listener, app)
                    .await
                    .expect("server error");
            });
        }
        Command::Analyze { smiles, basis } => {
            // Just RHF + partition analysis
            let atoms = match quasi_bridge::molecule::from_smiles(&smiles) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            let n_electrons = quasi_bridge::molecule::count_electrons(&atoms);

            println!("Molecule: {smiles}");
            println!("Atoms: {}", atoms.len());
            println!("Electrons: {n_electrons}");

            let bf = match quasi_bridge::basis::build_basis(&atoms, &basis) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            println!("Basis functions: {}", bf.len());

            let ints = quasi_bridge::integrals::compute_integrals(&atoms, &bf);
            println!("Nuclear repulsion: {:.6} Ha", ints.nuclear_repulsion);

            match quasi_bridge::rhf::rhf(&ints, n_electrons) {
                Ok(result) => {
                    println!(
                        "RHF energy: {:.6} Ha ({} iterations)",
                        result.energy, result.iterations
                    );
                    println!("Orbital energies:");
                    for (i, e) in result.orbital_energies.iter().enumerate() {
                        let occ = if i < result.n_occupied { "*" } else { " " };
                        println!("  {occ} MO {i}: {e:.6} Ha");
                    }
                }
                Err(e) => {
                    eprintln!("RHF error: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
