// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// Registers QuasiCircuitSimulator with NVQIR.
//
// NVQIR_REGISTER_SIMULATOR emits the extern "C" entry points that CUDA-Q's
// plugin loader resolves: getCircuitSimulator() and getCircuitSimulator_quasi().
// The printed name must match QuasiCircuitSimulator::name() and the library
// filename (libnvqir-quasi.so).

#include "QuasiCircuitSimulator.h"

NVQIR_REGISTER_SIMULATOR(nvqir::QuasiCircuitSimulator, quasi)
