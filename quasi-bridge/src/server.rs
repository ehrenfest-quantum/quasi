// SPDX-License-Identifier: AGPL-3.0-or-later
//! HTTP server for quasi-bridge.
//!
//! Endpoints — contract is the one Garm's `garm-engine/src/quasi_client.rs`
//! already declares (so adding this server unblocks both Garm and the
//! warhead kata):
//!
//! - `GET  /health`            — liveness + counters
//! - `POST /analyze`           — RHF + active-space partition (no quantum exec)
//! - `POST /run`               — full SMILES → ground-state energy pipeline
//! - `POST /run_active_space`  — pre-computed integrals → quantum kernel energy
//!
//! `/run_active_space` is the leg the Arn `warhead_classifier` kata invokes
//! at its `execute` stage. It accepts `{h1_eff, h2_eff, n_active,
//! n_active_elec, nuclear_repulsion, sword_json, backend?, huoma_chi?}`,
//! runs JW + Ehrenfest + Afana + ground-state diagonalization, and
//! returns a result that matches `MolecularResult` for downstream stitching.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ehrenfest_mol::{self, Accuracy};
use crate::jordan_wigner;
use crate::pipeline::{self, PipelineConfig};
use crate::postprocess;

#[derive(Clone)]
pub struct AppState {
    started_at: Instant,
    pub requests_served: Arc<AtomicU64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            requests_served: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/analyze", post(analyze))
        .route("/run", post(run))
        .route("/run_active_space", post(run_active_space))
        .with_state(state)
}

// ── /health ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: u64,
    requests_served: u64,
}

async fn health(State(s): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        uptime_seconds: s.started_at.elapsed().as_secs(),
        requests_served: s.requests_served.load(Ordering::Relaxed),
    })
}

// ── /analyze ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnalyzeRequest {
    smiles: String,
    #[serde(default)]
    basis: Option<String>,
}

#[derive(Debug, Serialize)]
struct AnalyzeResponse {
    smiles: String,
    n_atoms: usize,
    n_electrons: usize,
    n_basis_functions: usize,
    nuclear_repulsion: f64,
    rhf_energy: f64,
    rhf_iterations: usize,
    active_orbitals: Vec<usize>,
    n_electrons_active: usize,
    n_qubits: usize,
    partition_method: String,
    orbital_energies: Vec<f64>,
}

async fn analyze(
    State(s): State<AppState>,
    Json(req): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, ApiError> {
    s.requests_served.fetch_add(1, Ordering::Relaxed);

    let basis = req.basis.unwrap_or_else(|| "sto-3g".to_string());

    let atoms = crate::molecule::from_smiles(&req.smiles)
        .map_err(|e| ApiError::bad_request(format!("molecule: {e}")))?;
    let n_electrons = crate::molecule::count_electrons(&atoms);
    let bf = crate::basis::build_basis(&atoms, &basis)
        .map_err(|e| ApiError::bad_request(format!("basis: {e}")))?;
    let n_spatial = bf.len();
    let ints = crate::integrals::compute_integrals(&atoms, &bf);

    let rhf_result = crate::rhf::rhf(&ints, n_electrons)
        .map_err(|e| ApiError::internal(format!("rhf: {e}")))?;

    let part = crate::partition::frontier_partition(n_spatial, n_electrons, None)
        .map_err(|e| ApiError::internal(format!("partition: {e}")))?;

    Ok(Json(AnalyzeResponse {
        smiles: req.smiles,
        n_atoms: atoms.len(),
        n_electrons,
        n_basis_functions: n_spatial,
        nuclear_repulsion: ints.nuclear_repulsion,
        rhf_energy: rhf_result.energy,
        rhf_iterations: rhf_result.iterations,
        active_orbitals: part.active.clone(),
        n_electrons_active: part.n_electrons_active,
        n_qubits: 2 * part.active.len(),
        partition_method: part.method.clone(),
        orbital_energies: rhf_result.orbital_energies.iter().copied().collect(),
    }))
}

// ── /run ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RunRequest {
    smiles: String,
    #[serde(default)]
    basis: Option<String>,
    #[serde(default)]
    accuracy: Option<String>,
    #[serde(default)]
    n_active: Option<usize>,
    #[serde(default)]
    garm_partition: Option<Value>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    energy: f64,
    rhf_energy: f64,
    correlation: f64,
    error_bound_mha: f64,
    n_qubits: usize,
    n_pauli_terms: usize,
    backend: String,
    trotter_steps: u32,
    gate_count: usize,
    smiles: String,
    basis: String,
}

async fn run(
    State(s): State<AppState>,
    Json(req): Json<RunRequest>,
) -> Result<Json<RunResponse>, ApiError> {
    s.requests_served.fetch_add(1, Ordering::Relaxed);

    let basis = req.basis.unwrap_or_else(|| "sto-3g".to_string());
    let accuracy = match req.accuracy.as_deref().unwrap_or("chemical") {
        "chemical" => Accuracy::Chemical,
        "spectroscopic" => Accuracy::Spectroscopic,
        "exact" => Accuracy::Exact,
        other => return Err(ApiError::bad_request(format!("unknown accuracy: {other}"))),
    };
    let garm_json = req
        .garm_partition
        .as_ref()
        .map(|v| v.to_string());

    let cfg = PipelineConfig {
        basis: basis.clone(),
        accuracy,
        n_active: req.n_active,
        garm_json,
        verbose: false,
    };
    let result = pipeline::run_molecule(&req.smiles, &cfg)
        .map_err(|e| ApiError::internal(format!("pipeline: {e}")))?;

    Ok(Json(RunResponse {
        energy: result.energy,
        rhf_energy: result.rhf_energy,
        correlation: result.energy - result.rhf_energy,
        error_bound_mha: result.trotter_error_bound_mha,
        n_qubits: result.n_qubits,
        n_pauli_terms: result.n_pauli_terms,
        backend: result.backend,
        trotter_steps: result.trotter_steps,
        gate_count: result.gate_count,
        smiles: result.smiles,
        basis: result.basis,
    }))
}

// ── /run_active_space ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ActiveSpaceRequest {
    /// Active-space one-body integrals, flat row-major (n*n).
    h1_eff: Vec<f64>,
    /// Active-space two-body integrals, flat (n*n*n*n).
    h2_eff: Vec<f64>,
    /// Number of active spatial orbitals.
    n_active: usize,
    /// Number of active electrons.
    n_active_elec: usize,
    /// Nuclear repulsion energy (Ha).
    #[serde(default)]
    nuclear_repulsion: f64,
    /// Optional SWORD document (recorded for provenance, does not affect
    /// energy computation in this MVP).
    #[serde(default)]
    sword_json: Option<Value>,
    /// Backend hint — recorded for provenance only in this MVP.
    #[serde(default = "d_backend")]
    backend: String,
    /// MPS bond dimension for Huoma — recorded; not yet wired into the
    /// statevector path.
    #[serde(default)]
    huoma_chi: Option<u32>,
}

fn d_backend() -> String {
    "huoma_statevector".to_string()
}

#[derive(Debug, Serialize)]
struct ActiveSpaceResponse {
    energy: f64,
    n_qubits: usize,
    n_pauli_terms: usize,
    backend: String,
    nuclear_repulsion: f64,
    /// Echoed back so the kata can persist it.
    huoma_chi: Option<u32>,
    /// Wall time in seconds for the diagonalization leg.
    wall_time_s: f64,
}

async fn run_active_space(
    State(s): State<AppState>,
    Json(req): Json<ActiveSpaceRequest>,
) -> Result<Json<ActiveSpaceResponse>, ApiError> {
    s.requests_served.fetch_add(1, Ordering::Relaxed);

    let n = req.n_active;
    if req.h1_eff.len() != n * n {
        return Err(ApiError::bad_request(format!(
            "h1_eff length {} ≠ n_active² ({})",
            req.h1_eff.len(),
            n * n
        )));
    }
    if req.h2_eff.len() != n * n * n * n {
        return Err(ApiError::bad_request(format!(
            "h2_eff length {} ≠ n_active⁴ ({})",
            req.h2_eff.len(),
            n * n * n * n
        )));
    }

    // Reshape h1 into Vec<Vec<f64>> for jordan_wigner.
    let h_one: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| req.h1_eff[i * n + j]).collect())
        .collect();

    let pauli_terms = jordan_wigner::jordan_wigner_transform(
        &h_one,
        &req.h2_eff,
        n,
        req.nuclear_repulsion,
    );
    let n_qubits = 2 * n;

    // Build Ehrenfest program → Afana parse-only (matches /run flow); we
    // don't need to emit QASM here for the MVP.
    let program = ehrenfest_mol::build_ehrenfest_program(
        &pauli_terms,
        n_qubits,
        Accuracy::Chemical,
    );
    let cbor_bytes = ehrenfest_mol::to_cbor(&program);
    afana::cbor::from_cbor(&cbor_bytes)
        .map_err(|e| ApiError::internal(format!("afana cbor: {e}")))?;

    let started = Instant::now();
    let energy = postprocess::ground_state_energy(&pauli_terms, n_qubits)
        .map_err(|e| ApiError::internal(format!("ground_state: {e}")))?;
    let wall_time_s = started.elapsed().as_secs_f64();

    // Tag for provenance only; SWORD doc isn't load-bearing in MVP.
    let _ = req.sword_json;

    Ok(Json(ActiveSpaceResponse {
        energy,
        n_qubits,
        n_pauli_terms: pauli_terms.len(),
        backend: req.backend,
        nuclear_repulsion: req.nuclear_repulsion,
        huoma_chi: req.huoma_chi,
        wall_time_s,
    }))
}

// ── Error handling ────────────────────────────────────────────────────────

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}
