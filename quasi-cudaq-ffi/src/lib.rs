// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! C FFI bridge for CUDA-Q → QUASI integration.
//!
//! Exposes Huoma-equivalent circuit simulation and Solvayeur scheduling
//! to the C++ `QuasiCircuitSimulator` plugin (`libnvqir-quasi.so`).
//!
//! ```text
//! CUDA-Q (C++)  ──FFI──>  quasi-cudaq-ffi (Rust)
//!   │                          │
//!   │ CircuitSimulator API     ├── qasm.rs    (QASM parser)
//!   │                          ├── sim.rs     (statevector simulator)
//!   │                          └── bridge.rs  (Hamiltonian interception)
//!   │                              │
//!   └── libnvqir-quasi.so         ├── afana (ZX-calculus, Trotter)
//!                                  ├── quasi-bridge (JW, exact diag)
//!                                  └── quasi-solvayeur (ATW scheduling)
//! ```

pub mod bridge;
pub mod ffi;
pub mod qasm;
pub mod sim;
