// SPDX-License-Identifier: AGPL-3.0-or-later
//! HTTP server for quasi-bridge.
//!
//! Three endpoints:
//!   POST /run     — full pipeline (SMILES → energy)
//!   POST /analyze — RHF + partition only
//!   GET  /health  — liveness check

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::ehrenfest_mol::Accuracy;
use crate::pipeline::PipelineConfig;
use crate::{basis, integrals, molecule, partition, rhf};

// ── Shared state ────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    start_time: Instant,
    request_count: &'static AtomicU64,
}

// ── Request / response types ────────────────────────────────────

#[derive(Deserialize)]
pub struct RunRequest {
    pub smiles: String,
    #[serde(default = "default_basis")]
    pub basis: String,
    #[serde(default = "default_accuracy")]
    pub accuracy: String,
    pub n_active: Option<usize>,
    pub garm_partition: Option<serde_json::Value>,
}

fn default_basis() -> String {
    "sto-3g".into()
}
fn default_accuracy() -> String {
    "chemical".into()
}

#[derive(Serialize)]
pub struct RunResponse {
    pub energy: f64,
    pub rhf_energy: f64,
    pub correlation: f64,
    pub error_bound_mha: f64,
    pub n_qubits: usize,
    pub n_pauli_terms: usize,
    pub backend: String,
    pub trotter_steps: u32,
    pub gate_count: usize,
    pub smiles: String,
    pub basis: String,
}

#[derive(Deserialize)]
pub struct AnalyzeRequest {
    pub smiles: String,
    #[serde(default = "default_basis")]
    pub basis: String,
}

#[derive(Serialize)]
pub struct AnalyzeResponse {
    pub smiles: String,
    pub n_atoms: usize,
    pub n_electrons: usize,
    pub n_basis_functions: usize,
    pub nuclear_repulsion: f64,
    pub rhf_energy: f64,
    pub rhf_iterations: usize,
    pub active_orbitals: Vec<usize>,
    pub n_electrons_active: usize,
    pub n_qubits: usize,
    pub partition_method: String,
    pub orbital_energies: Vec<f64>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub requests_served: u64,
}

// ── Router ──────────────────────────────────────────────────────

pub fn app() -> Router {
    static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

    let state = AppState {
        start_time: Instant::now(),
        request_count: &REQUEST_COUNT,
    };

    Router::new()
        .route("/run", post(handle_run))
        .route("/analyze", post(handle_analyze))
        .route("/health", get(handle_health))
        .with_state(state)
}

// ── Handlers ────────────────────────────────────────────────────

async fn handle_run(
    State(state): State<AppState>,
    Json(req): Json<RunRequest>,
) -> impl IntoResponse {
    state.request_count.fetch_add(1, Ordering::Relaxed);

    let accuracy = match req.accuracy.as_str() {
        "chemical" => Accuracy::Chemical,
        "spectroscopic" => Accuracy::Spectroscopic,
        "exact" => Accuracy::Exact,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("unknown accuracy: {other}")})),
            );
        }
    };

    let garm_json = req
        .garm_partition
        .map(|v| serde_json::to_string(&v).unwrap_or_default());

    let config = PipelineConfig {
        basis: req.basis,
        accuracy,
        n_active: req.n_active,
        garm_json,
        verbose: false,
    };

    // Run pipeline on blocking thread (heavy linear algebra)
    let smiles = req.smiles.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::pipeline::run_molecule(&smiles, &config)
    })
    .await;

    match result {
        Ok(Ok(r)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "energy": r.energy,
                "rhf_energy": r.rhf_energy,
                "correlation": r.energy - r.rhf_energy,
                "error_bound_mha": r.trotter_error_bound_mha,
                "n_qubits": r.n_qubits,
                "n_pauli_terms": r.n_pauli_terms,
                "backend": r.backend,
                "trotter_steps": r.trotter_steps,
                "gate_count": r.gate_count,
                "smiles": r.smiles,
                "basis": r.basis,
            })),
        ),
        Ok(Err(e)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("task panicked: {e}")})),
        ),
    }
}

async fn handle_analyze(
    State(state): State<AppState>,
    Json(req): Json<AnalyzeRequest>,
) -> impl IntoResponse {
    state.request_count.fetch_add(1, Ordering::Relaxed);

    let smiles = req.smiles.clone();
    let basis_name = req.basis.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<AnalyzeResponse, String> {
        let atoms = molecule::from_smiles(&smiles).map_err(|e| e.to_string())?;
        let n_electrons = molecule::count_electrons(&atoms);
        let bf = basis::build_basis(&atoms, &basis_name).map_err(|e| e.to_string())?;
        let n_basis_functions = bf.len();
        let ints = integrals::compute_integrals(&atoms, &bf);
        let rhf_result = rhf::rhf(&ints, n_electrons).map_err(|e| e.to_string())?;
        let part = partition::frontier_partition(n_basis_functions, n_electrons, None)
            .map_err(|e| e.to_string())?;

        Ok(AnalyzeResponse {
            smiles,
            n_atoms: atoms.len(),
            n_electrons,
            n_basis_functions,
            nuclear_repulsion: ints.nuclear_repulsion,
            rhf_energy: rhf_result.energy,
            rhf_iterations: rhf_result.iterations,
            active_orbitals: part.active.clone(),
            n_electrons_active: part.n_electrons_active,
            n_qubits: 2 * part.active.len(),
            partition_method: part.method.clone(),
            orbital_energies: rhf_result.orbital_energies.iter().copied().collect(),
        })
    })
    .await;

    match result {
        Ok(Ok(resp)) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Ok(Err(e)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("task panicked: {e}")})),
        ),
    }
}

async fn handle_health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        requests_served: state.request_count.load(Ordering::Relaxed),
    })
}
