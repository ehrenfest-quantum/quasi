// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// C FFI declarations for Solvayeur quantum kernel (backed by quasi-cudaq-ffi Rust crate).

#ifndef QUASI_CUDAQ_SOLVAYEUR_FFI_H
#define QUASI_CUDAQ_SOLVAYEUR_FFI_H

#include <cstddef>
#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

// ── Solvayeur Handle ────────────────────────────────────────────────────────

/// Opaque handle to the Solvayeur kernel state.
typedef struct SolvayeurHandle SolvayeurHandle;

/// Create a Solvayeur handle in classical-fallback mode.
SolvayeurHandle* solvayeur_create_classical(void);

/// Destroy a Solvayeur handle.
void solvayeur_destroy(SolvayeurHandle* handle);

/// Ask the Solvayeur which backend to use.
/// Returns the selected backend index, or 0 on error.
int solvayeur_decide(SolvayeurHandle* handle);

/// Observe a reward and update the Solvayeur's bias fields.
void solvayeur_observe(
    SolvayeurHandle* handle,
    int backend_idx,
    double fidelity,
    double latency_ms,
    double cost
);

// ── Bridge FFI (Level 2: Hamiltonian Interception) ──────────────────────────

/// C-compatible Pauli term.
typedef struct {
    double coefficient;
    const char* pauli_string;  // null-terminated Pauli string, e.g. "XYZII"
} FfiPauliTerm;

/// Compute the ground-state energy for a Pauli Hamiltonian.
/// Returns NaN on error.
double quasi_bridge_observe(
    const FfiPauliTerm* pauli_terms,
    size_t n_terms,
    size_t n_qubits
);

#ifdef __cplusplus
}
#endif

#endif // QUASI_CUDAQ_SOLVAYEUR_FFI_H
