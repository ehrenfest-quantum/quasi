// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// Level 2: Hamiltonian Interception for cudaq::observe().
//
// When CUDA-Q calls observe() with a spin_op Hamiltonian, we intercept
// it before CUDA-Q decomposes it into circuits. The Pauli terms are
// extracted and routed through QUASI's pipeline:
//
//   spin_op → Pauli terms → quasi_bridge_observe() [FFI]
//       → Ehrenfest → Afana (Trotter, ZX) → exact diag → energy
//
// This gives CUDA-Q users automatic active-space partitioning and
// ZX-optimised circuits — something CUDA-Q itself doesn't offer.

#include "SolvayeurFFI.h"

#include <string>
#include <utility>
#include <vector>

namespace quasi {

/// Intercept a Hamiltonian expressed as Pauli terms and compute its
/// ground-state energy through QUASI's pipeline.
///
/// @param pauli_terms  Vector of (coefficient, pauli_string) pairs
/// @param n_qubits     Number of qubits in the system
/// @return             Ground-state energy, or NaN on error
double observe_hamiltonian(
    const std::vector<std::pair<double, std::string>>& pauli_terms,
    std::size_t n_qubits
) {
    if (pauli_terms.empty() || n_qubits == 0) {
        return __builtin_nan("");
    }

    // Convert to FFI format
    std::vector<FfiPauliTerm> ffi_terms;
    ffi_terms.reserve(pauli_terms.size());
    for (const auto& [coeff, pauli_str] : pauli_terms) {
        ffi_terms.push_back({coeff, pauli_str.c_str()});
    }

    // Call through FFI to QUASI's pipeline
    return quasi_bridge_observe(
        ffi_terms.data(),
        ffi_terms.size(),
        n_qubits
    );
}

} // namespace quasi
