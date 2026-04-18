// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Solvayeur quantum kernel — QPU-native resource scheduler.
//!
//! The Solvayeur uses the ATW (Around The World) algorithm to select quantum
//! backends for workload dispatch. It encodes the scheduling problem as a
//! transverse-field Ising model, compiles it through Afana, and uses
//! measurement outcomes as dispatching decisions.
//!
//! ```text
//! Backends → ATW Hamiltonian → Afana (compile) → circuit
//!                                                    │
//!                                              measure → bitstring
//!                                                    │
//!                                              decode → backend index
//!                                                    │
//!                                              dispatch → observe reward
//!                                                    │
//!                                              update bias → next round
//! ```

pub mod atw;
pub mod bias;
pub mod calibration;
pub mod coherence;
pub mod entanglement;
pub mod error_budget;
pub mod epoch;
pub mod hamiltonian;
pub mod metrics;
pub mod partition;
pub mod qubit_map;
