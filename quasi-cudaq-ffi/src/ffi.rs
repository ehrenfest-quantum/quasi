// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! C FFI exports for the CUDA-Q CircuitSimulator adapter.
//!
//! Every pointer crossing the Rust/C++ boundary is null-checked.
//! Every string gets lifetime-validated. /doubleknuth on FFI safety.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::bridge;
use crate::qasm;
use crate::sim;

// ── Huoma Handle ────────────────────────────────────────────────────────────

/// Opaque handle to the Huoma simulator state.
pub struct HuomaHandle {
    _marker: (),
}

/// Opaque handle to a simulation result.
pub struct HuomaResult {
    counts: Vec<(String, u32)>,
    fidelity: f64,
    time_ms: f64,
}

/// Create a new Huoma simulator handle.
#[no_mangle]
pub extern "C" fn huoma_create() -> *mut HuomaHandle {
    Box::into_raw(Box::new(HuomaHandle { _marker: () }))
}

/// Destroy a Huoma simulator handle.
///
/// # Safety
/// `handle` must be a valid pointer from `huoma_create()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_destroy(handle: *mut HuomaHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Execute a QASM circuit on Huoma and return measurement samples.
///
/// # Safety
/// - `handle` must be a valid pointer from `huoma_create()`
/// - `qasm_str` must be a valid null-terminated UTF-8 C string
/// - `measure_qubits` must point to `n_measure` valid `usize` values
///
/// Returns null on parse/execution error.
#[no_mangle]
pub unsafe extern "C" fn huoma_execute(
    handle: *mut HuomaHandle,
    qasm_str: *const c_char,
    measure_qubits: *const usize,
    n_measure: usize,
    shots: i32,
) -> *mut HuomaResult {
    // Null checks — /doubleknuth: every pointer gets checked
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    if qasm_str.is_null() {
        return std::ptr::null_mut();
    }

    // Parse QASM string
    let qasm_cstr = match CStr::from_ptr(qasm_str).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let circuit = match qasm::parse_qasm(qasm_cstr) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    // Build measure qubit list
    let measure: Vec<usize> = if measure_qubits.is_null() || n_measure == 0 {
        (0..circuit.n_qubits).collect()
    } else {
        std::slice::from_raw_parts(measure_qubits, n_measure).to_vec()
    };

    let shots = if shots <= 0 { 1000 } else { shots as u32 };

    // Execute simulation
    let result = sim::simulate(&circuit, &measure, shots);

    Box::into_raw(Box::new(HuomaResult {
        counts: result.counts,
        fidelity: result.fidelity_estimate,
        time_ms: result.wall_time_ms,
    }))
}

/// Get the number of distinct measurement outcomes.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_count(result: *const HuomaResult) -> i32 {
    if result.is_null() {
        return 0;
    }
    (*result).counts.len() as i32
}

/// Get the bitstring for measurement outcome at index `i`.
///
/// Returns a null-terminated string. The caller must free this pointer
/// with the standard C `free()` or let it leak (small allocation).
///
/// Returns null if `i` is out of bounds or result is null.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_bitstring(
    result: *const HuomaResult,
    i: i32,
) -> *const c_char {
    if result.is_null() || i < 0 {
        return std::ptr::null();
    }
    let idx = i as usize;
    let counts = &(*result).counts;
    if idx >= counts.len() {
        return std::ptr::null();
    }
    let cstr = match CString::new(counts[idx].0.as_str()) {
        Ok(s) => s,
        Err(_) => return std::ptr::null(),
    };
    cstr.into_raw() as *const c_char
}

/// Get the frequency (count) for measurement outcome at index `i`.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_frequency(result: *const HuomaResult, i: i32) -> i32 {
    if result.is_null() || i < 0 {
        return 0;
    }
    let idx = i as usize;
    let counts = &(*result).counts;
    if idx >= counts.len() {
        return 0;
    }
    counts[idx].1 as i32
}

/// Get the fidelity estimate from the simulation.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_fidelity(result: *const HuomaResult) -> f64 {
    if result.is_null() {
        return 0.0;
    }
    (*result).fidelity
}

/// Get the wall-clock time of the simulation in milliseconds.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_time_ms(result: *const HuomaResult) -> f64 {
    if result.is_null() {
        return 0.0;
    }
    (*result).time_ms
}

/// Destroy a Huoma result.
///
/// # Safety
/// `result` must be a valid pointer from `huoma_execute()`, or null.
#[no_mangle]
pub unsafe extern "C" fn huoma_result_destroy(result: *mut HuomaResult) {
    if !result.is_null() {
        drop(Box::from_raw(result));
    }
}

// ── Solvayeur Handle ────────────────────────────────────────────────────────

/// Opaque handle to the Solvayeur kernel state.
pub struct SolvayeurHandle {
    solvayeur: quasi_solvayeur::atw::Solvayeur,
}

/// Create a Solvayeur handle in classical-fallback mode.
#[no_mangle]
pub extern "C" fn solvayeur_create_classical() -> *mut SolvayeurHandle {
    let backends = make_default_backends();
    Box::into_raw(Box::new(SolvayeurHandle {
        solvayeur: quasi_solvayeur::atw::Solvayeur::new(&backends),
    }))
}

/// Destroy a Solvayeur handle.
///
/// # Safety
/// `handle` must be a valid pointer from `solvayeur_create_classical()`, or null.
#[no_mangle]
pub unsafe extern "C" fn solvayeur_destroy(handle: *mut SolvayeurHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Ask the Solvayeur which backend to use.
///
/// Returns the selected backend index, or 0 on error.
///
/// # Safety
/// `handle` must be a valid pointer from `solvayeur_create_classical()`.
#[no_mangle]
pub unsafe extern "C" fn solvayeur_decide(handle: *mut SolvayeurHandle) -> i32 {
    if handle.is_null() {
        return 0;
    }
    match (*handle).solvayeur.decide() {
        Ok(result) => result.backend_index as i32,
        Err(_) => 0,
    }
}

/// Observe a reward and update the Solvayeur's bias fields.
///
/// # Safety
/// `handle` must be a valid pointer from `solvayeur_create_classical()`.
#[no_mangle]
pub unsafe extern "C" fn solvayeur_observe(
    handle: *mut SolvayeurHandle,
    _backend_idx: i32,
    fidelity: f64,
    latency_ms: f64,
    cost: f64,
) {
    if handle.is_null() {
        return;
    }
    let reward = quasi_solvayeur::bias::Reward {
        fidelity,
        latency_ms,
        cost,
    };
    // Re-run decide to get the epoch result for observe
    if let Ok(epoch_result) = (*handle).solvayeur.decide() {
        (*handle).solvayeur.observe(&epoch_result, reward);
    }
}

// ── Bridge FFI (Level 2: Hamiltonian Interception) ──────────────────────────

/// C-compatible Pauli term for the bridge API.
#[repr(C)]
pub struct FfiPauliTerm {
    pub coefficient: f64,
    pub pauli_string: *const c_char,
}

/// Compute the ground-state energy for a Pauli Hamiltonian.
///
/// This is the Level 2 "Hamiltonian interceptor" — routes the Hamiltonian
/// through QUASI's pipeline (Ehrenfest → Afana → exact diag) instead of
/// letting CUDA-Q decompose it into circuits.
///
/// Returns NaN on error.
///
/// # Safety
/// - `pauli_terms` must point to `n_terms` valid `FfiPauliTerm` structs
/// - Each `pauli_string` in the terms must be a valid null-terminated UTF-8 string
#[no_mangle]
pub unsafe extern "C" fn quasi_bridge_observe(
    pauli_terms: *const FfiPauliTerm,
    n_terms: usize,
    n_qubits: usize,
) -> f64 {
    if pauli_terms.is_null() || n_terms == 0 {
        return f64::NAN;
    }

    let terms_slice = std::slice::from_raw_parts(pauli_terms, n_terms);

    // Convert C Pauli terms to Rust
    let mut rust_terms: Vec<(f64, String)> = Vec::with_capacity(n_terms);
    for term in terms_slice {
        if term.pauli_string.is_null() {
            return f64::NAN;
        }
        let pauli_str = match CStr::from_ptr(term.pauli_string).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return f64::NAN,
        };
        rust_terms.push((term.coefficient, pauli_str));
    }

    bridge::observe_hamiltonian(&rust_terms, n_qubits).unwrap_or(f64::NAN)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_default_backends() -> Vec<quasi_scheduler::backend::Backend> {
    use quasi_scheduler::backend::*;
    vec![Backend {
        capabilities: Capabilities::simulator(30),
        backend_type: BackendType::Simulator,
        status: BackendStatus::Online { queue_depth: 0 },
        cost_per_shot: 0.0001,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huoma_create_and_destroy() {
        let handle = huoma_create();
        assert!(!handle.is_null());
        unsafe {
            huoma_destroy(handle);
        }
    }

    #[test]
    fn solvayeur_create_and_destroy() {
        let handle = solvayeur_create_classical();
        assert!(!handle.is_null());
        unsafe {
            let idx = solvayeur_decide(handle);
            assert!(idx >= 0);
            solvayeur_destroy(handle);
        }
    }

    #[test]
    fn huoma_execute_bell_state() {
        let handle = huoma_create();
        let qasm = CString::new(
            "OPENQASM 2.0;\nqreg q[2];\ncreg c[2];\nh q[0];\ncx q[0],q[1];\n",
        )
        .unwrap();
        let measure_qubits: Vec<usize> = vec![0, 1];

        unsafe {
            let result = huoma_execute(
                handle,
                qasm.as_ptr(),
                measure_qubits.as_ptr(),
                measure_qubits.len(),
                1000,
            );
            assert!(!result.is_null());

            let count = huoma_result_count(result);
            assert_eq!(count, 2, "Bell state should have 2 outcomes");

            let fid = huoma_result_fidelity(result);
            assert!((fid - 1.0).abs() < 1e-10);

            huoma_result_destroy(result);
            huoma_destroy(handle);
        }
    }

    #[test]
    fn huoma_execute_null_handle_returns_null() {
        unsafe {
            let qasm = CString::new("OPENQASM 2.0;\nqreg q[1];\n").unwrap();
            let result =
                huoma_execute(std::ptr::null_mut(), qasm.as_ptr(), std::ptr::null(), 0, 100);
            assert!(result.is_null());
        }
    }

    #[test]
    fn huoma_execute_null_qasm_returns_null() {
        let handle = huoma_create();
        unsafe {
            let result = huoma_execute(handle, std::ptr::null(), std::ptr::null(), 0, 100);
            assert!(result.is_null());
            huoma_destroy(handle);
        }
    }

    #[test]
    fn bridge_observe_simple_hamiltonian() {
        let terms = vec![FfiPauliTerm {
            coefficient: 1.0,
            pauli_string: CString::new("Z").unwrap().into_raw(),
        }];
        unsafe {
            let energy = quasi_bridge_observe(terms.as_ptr(), terms.len(), 1);
            assert!((energy - (-1.0)).abs() < 1e-10, "energy = {energy}");
            // Clean up CStrings
            for term in &terms {
                drop(CString::from_raw(term.pauli_string as *mut c_char));
            }
        }
    }

    #[test]
    fn bridge_observe_null_returns_nan() {
        unsafe {
            let energy = quasi_bridge_observe(std::ptr::null(), 0, 1);
            assert!(energy.is_nan());
        }
    }
}
