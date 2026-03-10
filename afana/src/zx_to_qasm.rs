// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QU QUASI Contributors
//! Conversion from ZX-IR spiders to QASM statements.
//!
//! This module provides a minimal mapping from ZX spider types to QASM gate
//! emissions, sufficient for the current test suite. In particular, it adds
//! support for the Hadamard (H) spider, which emits a QASM3 `h` statement.

use crate::ast::GateName;
use crate::error::EmitError;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

/// ZX spider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZXType {
    Z,
    X,
    H,
}

/// Emit a non‑parametric gate.
fn emit_gate(gate: GateName, qubits: &[usize]) -> Result<String, EmitError> {
    let args = qubits
        .iter()
        .map(|i| format!("q[{}]", i))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("{} {};", gate.as_str(), args))
}

/// Emit a parametric gate (currently only used for Rz).
fn emit_param_gate(gate: GateName, param: f64, qubits: &[usize]) -> Result<String, EmitError> {
    let args = qubits
        .iter()
        .map(|i| format!("q[{}]", i))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("{}({}) {};", gate.as_str(), format_float(param), args))
}

/// Convert a ZX spider into a QASM gate string.
///
/// * `spider_type` – the type of the spider (Z, X, or H).
/// * `simplified_phase` – optional phase that has been reduced to a multiple of π.
/// * `phase_param` – raw phase parameter for parametric gates (e.g., Rz).
/// * `qubits` – the qubits the spider acts on.
pub fn emit_spider(
    spider_type: ZXType,
    simplified_phase: Option<f64>,
    phase_param: Option<f64>,
    qubits: Vec<usize>,
) -> Result<String, EmitError> {
    match spider_type {
        ZXType::Z => {
            if let Some(angle) = simplified_phase {
                if (angle - FRAC_PI_2).abs() < 1e-10 {
                    return emit_gate(GateName::S, &qubits);
                }
                if (angle - FRAC_PI_4).abs() < 1e-10 {
                    return emit_gate(GateName::T, &qubits);
                }
            }
            if let Some(p) = phase_param {
                return emit_param_gate(GateName::Rz, p, &qubits);
            }
            // Default Z spider without phase.
            emit_gate(GateName::Z, &qubits)
        }
        ZXType::X => {
            // For X spiders we emit an H‑basis change followed by an Rz if a phase is present.
            // This simple implementation mirrors the existing trotterization behaviour.
            // Emit the H basis‑change.
            let h_gate = emit_gate(GateName::H, &qubits)?;
            if let Some(p) = phase_param {
                let rz_gate = emit_param_gate(GateName::Rz, p, &qubits)?;
                Ok(format!("{}\n{}", h_gate, rz_gate))
            } else {
                Ok(h_gate)
            }
        }
        ZXType::H => {
            // Hadamard spider directly maps to an H gate.
            emit_gate(GateName::H, &qubits)
        }
    }
}

/// Format a float for QASM output, using π‑fraction notation when appropriate.
fn format_float(val: f64) -> String {
    let ratio = val / PI;
    if (ratio - ratio.round()).abs() < 1e-10 {
        let r = ratio.round() as i64;
        return match r {
            0 => "0".into(),
            1 => "pi".into(),
            -1 => "-pi".into(),
            _ => format!("{}*pi", r),
        };
    }
    for denom in [2, 4, 8] {
        let numer = ratio * denom as f64;
        if (numer - numer.round()).abs() < 1e-10 {
            let n = numer.round() as i64;
            return match n {
                1 => format!("pi/{}", denom),
                -1 => format!("-pi/{}", denom),
                _ => format!("{}*pi/{}", n, denom),
            };
        }
    }
    format!("{val}")
}
