// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// QuasiCircuitSimulator — CUDA-Q backend plugin for QUASI.
//
// This file implements the CUDA-Q CircuitSimulator plugin interface.
// When compiled into libnvqir-quasi.so and registered via the quasi.config
// target file, CUDA-Q routes all circuit execution through QUASI's Huoma
// simulator via the Rust FFI bridge.
//
// Build: cmake .. && make
// Register: cudaq.set_target("quasi")
//
// The hot path is Huoma (Rust). This C++ layer is a thin shim that
// converts CUDA-Q's per-gate callbacks into an accumulated circuit,
// emits QASM, and calls through FFI.

#include "QuasiCircuitSimulator.h"

#include <iostream>

// ── Plugin Registration ─────────────────────────────────────────────────────
//
// In a real CUDA-Q installation, this would use NVQIR_REGISTER_SIMULATOR
// to register the backend. For now, we provide a factory function that
// CUDA-Q's plugin loader can dlsym().

extern "C" {

/// Factory function for CUDA-Q plugin loading.
/// CUDA-Q calls this to instantiate the simulator backend.
void* quasi_simulator_create() {
    return new quasi::QuasiCircuitSimulator();
}

/// Cleanup function for CUDA-Q plugin unloading.
void quasi_simulator_destroy(void* sim) {
    delete static_cast<quasi::QuasiCircuitSimulator*>(sim);
}

/// Return the name of this simulator backend.
const char* quasi_simulator_name() {
    return "quasi";
}

} // extern "C"
