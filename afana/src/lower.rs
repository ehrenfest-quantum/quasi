// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! AST-to-ZX-IR lowering pass.
//!
//! Converts an [`EhrenfestAst`] (gate-level circuit) into a [`ZxGraph`]
//! (ZX calculus graph). Each qubit becomes a wire through the graph:
//! input boundary → spider chain → output boundary.
//!
//! Gate decompositions follow the standard ZX calculus:
//!
//! | Gate    | ZX decomposition                              |
//! |---------|-----------------------------------------------|
//! | H       | Z(0) — X(0) — Z(0)                           |
//! | X       | X(1) single spider                            |
//! | Y       | Z(1) — X(1) (decomposed as Z·X)              |
//! | Z       | Z(1) single spider                            |
//! | S       | Z(1/2) single spider                          |
//! | T       | Z(1/4) single spider                          |
//! | Sdg     | Z(-1/2) single spider                         |
//! | Tdg     | Z(-1/4) single spider                         |
//! | Rx(θ)   | X(θ/π) single spider                          |
//! | Ry(θ)   | Z(1/2) — X(θ/π) — Z(-1/2)                    |
//! | Rz(θ)   | Z(θ/π) single spider                          |
//! | CX      | Z(0) arity-3 — X(0) arity-3, connected       |
//! | CZ      | Z(0) arity-3 — Z(0) arity-3, connected       |
//! | Swap    | Three CX gates decomposed                     |
//! | CCX     | Decomposed to 15-gate Clifford+T, then lowered|

use thiserror::Error;

use crate::ast::{EhrenfestAst, Gate, GateName};
use crate::decompose;
use crate::zx_ir::{NodeId, SpiderColor, ZxGraph};

/// Errors raised during AST-to-ZX lowering.
#[derive(Debug, Error)]
pub enum LowerError {
    #[error("unsupported gate: {0}")]
    UnsupportedGate(String),

    #[error("qubit index {index} out of range in gate `{gate}` (n_qubits={n_qubits})")]
    QubitOutOfRange {
        gate: String,
        index: usize,
        n_qubits: usize,
    },
}

/// Per-qubit wire state during lowering.
///
/// Tracks the most recent node on each qubit's wire so that new spiders
/// can be chained onto the correct predecessor.
struct WireHead {
    /// The node ID of the current tip of this qubit's wire.
    tip: NodeId,
}

/// Lower an [`EhrenfestAst`] into a [`ZxGraph`].
///
/// Each qubit gets an input boundary spider and an output boundary spider.
/// Gates are decomposed into spiders chained along each qubit's wire.
pub fn lower_ast_to_zx(ast: &EhrenfestAst) -> Result<ZxGraph, LowerError> {
    let mut graph = ZxGraph::new();
    let n = ast.n_qubits;

    // Create input boundary nodes (one Z(0) per qubit).
    let mut wires: Vec<WireHead> = Vec::with_capacity(n);
    let mut input_nodes = Vec::with_capacity(n);

    for q in 0..n {
        let inp = graph.add_spider(SpiderColor::Z, 0.0, Some(q));
        input_nodes.push(inp);
        wires.push(WireHead { tip: inp });
    }
    graph.set_inputs(input_nodes);

    // Lower each gate.
    for gate in &ast.gates {
        // Validate qubit indices.
        for &idx in &gate.qubits {
            if idx >= n {
                return Err(LowerError::QubitOutOfRange {
                    gate: gate.name.as_str().to_string(),
                    index: idx,
                    n_qubits: n,
                });
            }
        }

        lower_gate(&mut graph, &mut wires, gate)?;
    }

    // Create output boundary nodes and connect them.
    let mut output_nodes = Vec::with_capacity(n);
    for (q, wire) in wires.iter().enumerate() {
        let out = graph.add_spider(SpiderColor::Z, 0.0, Some(q));
        graph.add_edge(wire.tip, out);
        output_nodes.push(out);
    }
    graph.set_outputs(output_nodes);

    Ok(graph)
}

/// Lower a single gate into ZX spiders on the graph.
fn lower_gate(
    graph: &mut ZxGraph,
    wires: &mut [WireHead],
    gate: &Gate,
) -> Result<(), LowerError> {
    match &gate.name {
        // ── Single-qubit gates ───────────────────────────────────────

        // H = Z(0) — X(0) — Z(0)
        GateName::H => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 0.0);
            chain_spider(graph, wires, q, SpiderColor::X, 0.0);
            chain_spider(graph, wires, q, SpiderColor::Z, 0.0);
        }

        // X = X(π) → phase = 1.0 (in multiples of π)
        GateName::X => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::X, 1.0);
        }

        // Y = Z(π) · X(π) → Z(1) then X(1)
        GateName::Y => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 1.0);
            chain_spider(graph, wires, q, SpiderColor::X, 1.0);
        }

        // Z = Z(π) → phase = 1.0
        GateName::Z => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 1.0);
        }

        // S = Z(π/2) → phase = 0.5
        GateName::S => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 0.5);
        }

        // T = Z(π/4) → phase = 0.25
        GateName::T => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 0.25);
        }

        // Sdg = Z(-π/2) → phase = -0.5
        GateName::Sdg => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, -0.5);
        }

        // Tdg = Z(-π/4) → phase = -0.25
        GateName::Tdg => {
            let q = gate.qubits[0];
            chain_spider(graph, wires, q, SpiderColor::Z, -0.25);
        }

        // Rx(θ) = X(θ/π) single spider
        GateName::Rx => {
            let q = gate.qubits[0];
            let theta = gate.params[0];
            chain_spider(graph, wires, q, SpiderColor::X, theta / std::f64::consts::PI);
        }

        // Ry(θ) = Z(π/2) — X(θ/π) — Z(-π/2)
        GateName::Ry => {
            let q = gate.qubits[0];
            let theta = gate.params[0];
            chain_spider(graph, wires, q, SpiderColor::Z, 0.5);
            chain_spider(
                graph,
                wires,
                q,
                SpiderColor::X,
                theta / std::f64::consts::PI,
            );
            chain_spider(graph, wires, q, SpiderColor::Z, -0.5);
        }

        // Rz(θ) = Z(θ/π) single spider
        GateName::Rz => {
            let q = gate.qubits[0];
            let theta = gate.params[0];
            chain_spider(graph, wires, q, SpiderColor::Z, theta / std::f64::consts::PI);
        }

        // ── Two-qubit gates ─────────────────────────────────────────

        // CX = Z(0) on control (arity 3) — X(0) on target (arity 3), connected
        GateName::Cx => {
            let ctrl = gate.qubits[0];
            let targ = gate.qubits[1];
            let z = chain_spider(graph, wires, ctrl, SpiderColor::Z, 0.0);
            let x = chain_spider(graph, wires, targ, SpiderColor::X, 0.0);
            graph.add_edge(z, x);
        }

        // CZ = Z(0) on q0 (arity 3) — Z(0) on q1 (arity 3), connected
        GateName::Cz => {
            let q0 = gate.qubits[0];
            let q1 = gate.qubits[1];
            let z0 = chain_spider(graph, wires, q0, SpiderColor::Z, 0.0);
            let z1 = chain_spider(graph, wires, q1, SpiderColor::Z, 0.0);
            graph.add_edge(z0, z1);
        }

        // Swap = CX(a,b) · CX(b,a) · CX(a,b)
        GateName::Swap => {
            let a = gate.qubits[0];
            let b = gate.qubits[1];

            // First CX(a, b)
            let z1 = chain_spider(graph, wires, a, SpiderColor::Z, 0.0);
            let x1 = chain_spider(graph, wires, b, SpiderColor::X, 0.0);
            graph.add_edge(z1, x1);

            // Second CX(b, a)
            let z2 = chain_spider(graph, wires, b, SpiderColor::Z, 0.0);
            let x2 = chain_spider(graph, wires, a, SpiderColor::X, 0.0);
            graph.add_edge(z2, x2);

            // Third CX(a, b)
            let z3 = chain_spider(graph, wires, a, SpiderColor::Z, 0.0);
            let x3 = chain_spider(graph, wires, b, SpiderColor::X, 0.0);
            graph.add_edge(z3, x3);
        }

        // Ccx (Toffoli) — decompose to Clifford+T, then lower each sub-gate.
        GateName::Ccx => {
            let ctrl1 = gate.qubits[0];
            let ctrl2 = gate.qubits[1];
            let target = gate.qubits[2];
            let sub_gates = decompose::decompose_ccx(ctrl1, ctrl2, target);
            for sg in &sub_gates {
                lower_gate(graph, wires, sg)?;
            }
        }
    }

    Ok(())
}

/// Chain a new spider onto a qubit wire: add the spider, connect it to
/// the current wire tip, and update the wire tip. Returns the new node ID.
fn chain_spider(
    graph: &mut ZxGraph,
    wires: &mut [WireHead],
    qubit: usize,
    color: SpiderColor,
    phase: f64,
) -> NodeId {
    let node = graph.add_spider(color, phase, Some(qubit));
    graph.add_edge(wires[qubit].tip, node);
    wires[qubit].tip = node;
    node
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::zx_ir::SpiderColor;

    /// Helper: build a minimal AST.
    fn make_ast(n_qubits: usize, gates: Vec<Gate>) -> EhrenfestAst {
        EhrenfestAst {
            name: "test".into(),
            n_qubits,
            prepare: None,
            gates,
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        }
    }

    #[test]
    fn lower_empty_circuit() {
        let ast = make_ast(2, vec![]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 2 inputs + 2 outputs = 4 spiders
        assert_eq!(zx.spider_count(), 4);
        // 2 edges (input→output per qubit)
        assert_eq!(zx.edge_count(), 2);
    }

    #[test]
    fn lower_h_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 1 input + 3 spiders (Z-X-Z for H) + 1 output = 5
        assert_eq!(zx.spider_count(), 5);
    }

    #[test]
    fn lower_x_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::X, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 1 input + 1 X(1) spider + 1 output = 3
        assert_eq!(zx.spider_count(), 3);
        // The middle spider should be X with phase 1.0
        assert_eq!(zx.spider(1).color, SpiderColor::X);
        assert!((zx.spider(1).phase - 1.0).abs() < 1e-10);
    }

    #[test]
    fn lower_z_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::Z, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider_count(), 3);
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - 1.0).abs() < 1e-10);
    }

    #[test]
    fn lower_s_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::S, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - 0.5).abs() < 1e-10);
    }

    #[test]
    fn lower_t_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::T, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - 0.25).abs() < 1e-10);
    }

    #[test]
    fn lower_sdg_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::Sdg, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn lower_tdg_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::Tdg, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - (-0.25)).abs() < 1e-10);
    }

    #[test]
    fn lower_rx_gate() {
        let theta = std::f64::consts::FRAC_PI_2;
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rx, qubits: vec![0], params: vec![theta] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 1 input + 1 X spider + 1 output = 3
        assert_eq!(zx.spider_count(), 3);
        assert_eq!(zx.spider(1).color, SpiderColor::X);
        assert!((zx.spider(1).phase - 0.5).abs() < 1e-10); // pi/2 / pi = 0.5
    }

    #[test]
    fn lower_ry_gate() {
        let theta = std::f64::consts::FRAC_PI_4;
        let ast = make_ast(1, vec![
            Gate { name: GateName::Ry, qubits: vec![0], params: vec![theta] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 1 input + 3 spiders (Z, X, Z for Ry) + 1 output = 5
        assert_eq!(zx.spider_count(), 5);
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - 0.5).abs() < 1e-10);
        assert_eq!(zx.spider(2).color, SpiderColor::X);
        assert!((zx.spider(2).phase - 0.25).abs() < 1e-10);
        assert_eq!(zx.spider(3).color, SpiderColor::Z);
        assert!((zx.spider(3).phase - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn lower_rz_gate() {
        let theta = std::f64::consts::PI;
        let ast = make_ast(1, vec![
            Gate { name: GateName::Rz, qubits: vec![0], params: vec![theta] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        assert_eq!(zx.spider_count(), 3);
        assert_eq!(zx.spider(1).color, SpiderColor::Z);
        assert!((zx.spider(1).phase - 1.0).abs() < 1e-10);
    }

    #[test]
    fn lower_cx_gate() {
        let ast = make_ast(2, vec![
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 2 inputs + Z(0) on q0 + X(0) on q1 + 2 outputs = 6 spiders
        assert_eq!(zx.spider_count(), 6);
        // Edges: input0→Z, input1→X, Z↔X (cross), Z→out0, X→out1 = 5
        assert_eq!(zx.edge_count(), 5);
    }

    #[test]
    fn lower_cz_gate() {
        let ast = make_ast(2, vec![
            Gate { name: GateName::Cz, qubits: vec![0, 1], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 2 inputs + Z(0) on q0 + Z(0) on q1 + 2 outputs = 6 spiders
        assert_eq!(zx.spider_count(), 6);
        // Edges: input0→Z0, input1→Z1, Z0↔Z1, Z0→out0, Z1→out1 = 5
        assert_eq!(zx.edge_count(), 5);
        // Both gate spiders are Z
        assert_eq!(zx.spider(2).color, SpiderColor::Z);
        assert_eq!(zx.spider(3).color, SpiderColor::Z);
    }

    #[test]
    fn lower_swap_gate() {
        let ast = make_ast(2, vec![
            Gate { name: GateName::Swap, qubits: vec![0, 1], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // Swap = 3 CX gates, each adds 2 spiders = 6 gate spiders
        // + 2 inputs + 2 outputs = 10
        assert_eq!(zx.spider_count(), 10);
    }

    #[test]
    fn lower_y_gate() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::Y, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 1 input + Z(1) + X(1) + 1 output = 4
        assert_eq!(zx.spider_count(), 4);
    }

    #[test]
    fn lower_ccx_decomposes_to_clifford_t() {
        let ast = make_ast(3, vec![
            Gate { name: GateName::Ccx, qubits: vec![0, 1, 2], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // 3 inputs + 3 outputs + spiders from 15-gate Clifford+T decomposition.
        // The decomposition has: 2 H (3 spiders each = 6), 4 T/Tdg (1 each = 4),
        // 3 T on ctrl/target (1 each = 3), 6 CX (2 each = 12) = 25 gate spiders
        // + 6 boundary = 31 total.
        assert!(zx.spider_count() > 6, "CCX should produce many spiders");
        assert!(zx.edge_count() > 3, "CCX should produce many edges");
    }

    #[test]
    fn lower_qubit_out_of_range() {
        let ast = make_ast(1, vec![
            Gate { name: GateName::H, qubits: vec![5], params: vec![] },
        ]);
        let err = lower_ast_to_zx(&ast).unwrap_err();
        assert!(matches!(err, LowerError::QubitOutOfRange { .. }));
    }

    #[test]
    fn lower_bell_circuit() {
        // H on q0, CX on q0→q1, measure both
        let ast = make_ast(2, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());

        // Input: 2 boundary
        // H on q0: 3 spiders (Z-X-Z)
        // CX: 2 spiders (Z on q0, X on q1)
        // Output: 2 boundary
        // Total: 2 + 3 + 2 + 2 = 9
        assert_eq!(zx.spider_count(), 9);
    }

    #[test]
    fn lower_multi_gate_circuit_validates() {
        // A circuit with various gate types on 3 qubits.
        let ast = make_ast(3, vec![
            Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            Gate { name: GateName::T, qubits: vec![1], params: vec![] },
            Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            Gate { name: GateName::Rz, qubits: vec![2], params: vec![1.0] },
            Gate { name: GateName::Cz, qubits: vec![1, 2], params: vec![] },
            Gate { name: GateName::S, qubits: vec![0], params: vec![] },
        ]);
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());
        // Just verify it's a reasonable size and passes validation.
        assert!(zx.spider_count() > 6); // at least inputs + outputs
        assert!(zx.edge_count() > 3);
    }

    #[test]
    fn zx_ir_spider_phases_scaled_by_evolution_time() {
        // Test that ZX-IR spider phases are correctly scaled by evolution_time / trotter_steps
        // when lowering a single-qubit Hamiltonian AST node.
        //
        // For H = h*X with h=0.5 GHz·rad, total_us=1.0, steps=10:
        //   dt = 0.1 us
        //   theta = h * dt = 0.05
        //   Rz parameter = 2*theta = 0.1
        //   ZX phase = theta/pi = 0.1/pi ≈ 0.03183
        use crate::cbor::{EhrenfestProgram, EvolutionTime, Hamiltonian, NoiseConstraint,
            Observable, PauliOp, PauliOpEntry, PauliTerm, SystemDef};
        use crate::trotter::{trotterize, TrotterOrder};

        let coefficient = 0.5;
        let total_us = 1.0;
        let steps: u32 = 10;
        let dt_us = total_us / steps as f64; // 0.1

        let program = EhrenfestProgram {
            version: 1,
            system: SystemDef {
                n_qubits: 1,
                cooling_profile: None,
                backend_hint: None,
            },
            hamiltonian: Hamiltonian {
                terms: vec![PauliTerm {
                    coefficient,
                    paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
                }],
                constant_offset: 0.0,
            },
            evolution: EvolutionTime {
                total_us,
                steps,
                dt_us,
            },
            observables: vec![Observable::SX { qubit: 0 }],
            noise: NoiseConstraint {
                t1_us: 100.0,
                t2_us: 50.0,
                gate_fidelity_min: None,
                readout_fidelity_min: None,
            },
        };

        // Trotterize to get gate sequence with scaled phases
        let ast = trotterize(&program, TrotterOrder::First);

        // Lower to ZX-IR
        let zx = lower_ast_to_zx(&ast).unwrap();
        assert!(zx.validate().is_ok());

        // For X Hamiltonian: H - Rz(2*h*dt) - H pattern
        // The Rz spider should have phase = (2*h*dt)/pi = 2*coefficient*dt_us/pi
        let expected_phase = (2.0 * coefficient * dt_us) / std::f64::consts::PI;

        // Find the Rz spider (X spider with non-zero, non-1 phase)
        let mut found_scaled_spider = false;
        for i in 0..zx.spider_count() {
            let spider = zx.spider(i);
            // The Rz gate becomes an X spider with phase = theta/pi
            if spider.color == SpiderColor::X
                && spider.phase.abs() > 0.01
                && spider.phase.abs() < 0.5
            {
                assert!(
                    (spider.phase - expected_phase).abs() < 1e-6,
                    "ZX spider phase {} does not match expected {} (h={}, dt={})",
                    spider.phase, expected_phase, coefficient, dt_us
                );
                found_scaled_spider = true;
            }
        }

        assert!(
            found_scaled_spider,
            "Should find scaled X spider with phase {} in ZX graph",
            expected_phase
        );
    }
}
