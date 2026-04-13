// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// C FFI declarations for Huoma simulator (backed by quasi-cudaq-ffi Rust crate).

#ifndef QUASI_CUDAQ_HUOMA_FFI_H
#define QUASI_CUDAQ_HUOMA_FFI_H

#include <cstddef>
#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

// ── Huoma Handle ────────────────────────────────────────────────────────────

/// Opaque handle to the Huoma simulator state.
typedef struct HuomaHandle HuomaHandle;

/// Opaque handle to a simulation result.
typedef struct HuomaResult HuomaResult;

/// Create a new Huoma simulator handle.
HuomaHandle* huoma_create(void);

/// Destroy a Huoma simulator handle.
void huoma_destroy(HuomaHandle* handle);

/// Execute a QASM circuit on Huoma and return measurement samples.
///
/// @param handle        Valid handle from huoma_create()
/// @param qasm_str      Null-terminated OpenQASM 2.0 string
/// @param measure_qubits Array of qubit indices to measure
/// @param n_measure     Length of measure_qubits array
/// @param shots         Number of measurement shots
/// @return              Result handle, or NULL on error
HuomaResult* huoma_execute(
    HuomaHandle* handle,
    const char* qasm_str,
    const size_t* measure_qubits,
    size_t n_measure,
    int shots
);

/// Get the number of distinct measurement outcomes.
int huoma_result_count(const HuomaResult* result);

/// Get the bitstring for measurement outcome at index i.
/// Returns null-terminated string owned by the result (do NOT free).
const char* huoma_result_bitstring(const HuomaResult* result, int i);

/// Get the frequency (count) for measurement outcome at index i.
int huoma_result_frequency(const HuomaResult* result, int i);

/// Get the fidelity estimate from the simulation.
double huoma_result_fidelity(const HuomaResult* result);

/// Get the wall-clock time of the simulation in milliseconds.
double huoma_result_time_ms(const HuomaResult* result);

/// Destroy a Huoma result.
void huoma_result_destroy(HuomaResult* result);

#ifdef __cplusplus
}
#endif

#endif // QUASI_CUDAQ_HUOMA_FFI_H
