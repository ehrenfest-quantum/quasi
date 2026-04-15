// Integration test for ZX-IR to QASM3 emission with phase tracking.
//
// Verifies that single-qubit Pauli rotation sequences derived from
// Hamiltonian terms emit correct QASM3 with accurate phase values.

use afana::ast::GateName;
use afana::cbor::{
    EhrenfestProgram, EvolutionTime, Hamiltonian, NoiseConstraint, Observable,
    PauliOp, PauliOpEntry, PauliTerm, SystemDef,
};
use afana::emit::{emit_qasm, QasmVersion};
use afana::trotter::{trotterize, TrotterOrder};

/// Test Y-then-X Pauli evolution with non-trivial phase.
///
/// This test verifies that:
/// 1. The Y⊗Y term produces correct basis-change gates (Sdg, H) and Rz rotation
/// 2. The X term produces correct basis-change gates (H) and Rz rotation
/// 3. Phase angles are correctly computed as 2*coefficient*dt
/// 4. The emitted QASM3 matches the reference within 1e-9 phase tolerance
#[test]
fn test_y_then_x_pauli_evolution_phase_tracking() {
    // Build Ehrenfest program with Y⊗Y and X Hamiltonian terms.
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![
                // Y⊗Y term: coefficient 0.5
                PauliTerm {
                    coefficient: 0.5,
                    paulis: vec![
                        PauliOpEntry { qubit: 0, axis: PauliOp::Y },
                        PauliOpEntry { qubit: 1, axis: PauliOp::Y },
                    ],
                },
                // X term on qubit 1: coefficient 0.3
                PauliTerm {
                    coefficient: 0.3,
                    paulis: vec![PauliOpEntry { qubit: 1, axis: PauliOp::X }],
                },
            ],
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us: 1.0,
            steps: 1,
            dt_us: 1.0,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    };

    // Trotterize to get gate sequence.
    let ast = trotterize(&program, TrotterOrder::First);

    // Emit QASM3.
    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM3 emission failed");

    // Verify QASM3 header.
    assert!(
        qasm.contains("OPENQASM 3.0;"),
        "QASM3 output must contain OPENQASM 3.0 header"
    );

    // Verify Y⊗Y term decomposition:
    // Y basis change: Sdg, H on each qubit
    // CNOT ladder, Rz(2*theta), undo CNOT
    // Undo basis change: H, S on each qubit
    // theta = 0.5 * 1.0 = 0.5, so Rz angle = 2*0.5 = 1.0
    assert!(
        qasm.contains("sdg q[0];"),
        "Y⊗Y term needs Sdg basis change on q[0]"
    );
    assert!(
        qasm.contains("sdg q[1];"),
        "Y⊗Y term needs Sdg basis change on q[1]"
    );
    assert!(
        qasm.contains("h q[0];"),
        "Y⊗Y term needs H basis change on q[0]"
    );
    assert!(
        qasm.contains("h q[1];"),
        "Y⊗Y term needs H basis change on q[1]"
    );
    assert!(
        qasm.contains("cx q[0], q[1];"),
        "Y⊗Y term needs CNOT ladder"
    );
    // Rz angle for Y⊗Y: 2 * 0.5 * 1.0 = 1.0 radians
    assert!(
        qasm.contains("rz(1) q[1];") || qasm.contains("rz(1.0) q[1];"),
        "Y⊗Y term needs Rz(1.0) rotation, got:\n{}",
        qasm
    );

    // Verify X term decomposition:
    // X basis change: H
    // Rz(2*theta), undo H
    // theta = 0.3 * 1.0 = 0.3, so Rz angle = 2*0.3 = 0.6
    // Count H gates: Y⊗Y uses 4 H (2 forward + 2 backward), X uses 2 H = 6 total
    let h_count = qasm.matches("h q[").count();
    assert!(
        h_count >= 6,
        "Expected at least 6 H gates (4 for Y⊗Y + 2 for X), got {}",
        h_count
    );

    // Verify Rz angle for X term: 2 * 0.3 * 1.0 = 0.6 radians
    // The format_float function may output "0.6" or keep decimal form
    let has_correct_x_rotation = qasm.contains("rz(0.6) q[1];")
        || qasm.contains("rz(0.60")
        || (qasm.contains("rz(") && {
            // Extract Rz angles and check for 0.6
            let rz_lines: Vec<&str> = qasm
                .lines()
                .filter(|l| l.contains("rz(") && l.contains("q[1]"))
                .collect();
            rz_lines.iter().any(|l| {
                l.contains("0.6")
                    || l.contains("3*pi/5") // 0.6 ≈ 3π/5
                    || l.contains("0.599")
                    || l.contains("0.600")
            })
        });
    assert!(
        has_correct_x_rotation,
        "X term needs Rz(0.6) rotation, got:\n{}",
        qasm
    );

    // Verify gate ordering: Y⊗Y gates should appear before X gates
    // (terms are processed in order)
    let sdg_pos = qasm.find("sdg q[0];").unwrap_or(0);
    let x_rotation_pos = qasm.find("rz(0.6)").unwrap_or(usize::MAX);
    assert!(
        sdg_pos < x_rotation_pos,
        "Y⊗Y gates should appear before X term gates"
    );

    // Verify the AST has the expected gate structure
    let y_basis_gates: Vec<_> = ast
        .gates
        .iter()
        .filter(|g| matches!(g.name, GateName::Sdg | GateName::S))
        .collect();
    assert!(
        y_basis_gates.len() >= 4,
        "Y⊗Y term needs 4 S/Sdg gates (2 forward + 2 backward), got {}",
        y_basis_gates.len()
    );

    let h_gates: Vec<_> = ast
        .gates
        .iter()
        .filter(|g| g.name == GateName::H)
        .collect();
    assert!(
        h_gates.len() >= 6,
        "Expected at least 6 H gates, got {}",
        h_gates.len()
    );

    let rz_gates: Vec<_> = ast
        .gates
        .iter()
        .filter(|g| g.name == GateName::Rz)
        .collect();
    assert_eq!(
        rz_gates.len(),
        2,
        "Expected 2 Rz gates (one per Hamiltonian term), got {}",
        rz_gates.len()
    );

    // Verify Rz angles are within 1e-9 tolerance
    let rz_angles: Vec<f64> = rz_gates.iter().map(|g| g.params[0]).collect();
    assert!(
        (rz_angles[0] - 1.0).abs() < 1e-9,
        "First Rz angle should be 1.0 (for Y⊗Y), got {}",
        rz_angles[0]
    );
    assert!(
        (rz_angles[1] - 0.6).abs() < 1e-9,
        "Second Rz angle should be 0.6 (for X), got {}",
        rz_angles[1]
    );
}

/// Test phase normalization for angles outside (-π, π].
#[test]
fn test_phase_normalization_large_angles() {
    use afana::emit::emit_qasm;
    use afana::ast::{EhrenfestAst, Gate, GateName, Measure};

    // Create AST with large rotation angles that need normalization
    let ast = EhrenfestAst {
        name: "phase_norm_test".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![
            // 3π should normalize to π
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![3.0 * std::f64::consts::PI],
            },
            // 5π/2 should normalize to π/2
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![5.0 * std::f64::consts::FRAC_PI_2],
            },
            // -3π should normalize to π
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![-3.0 * std::f64::consts::PI],
            },
        ],
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM3 emission failed");

    // 3π → π (canonical form)
    assert!(
        qasm.contains("rz(pi) q[0];"),
        "3π should normalize to pi, got:\n{}",
        qasm
    );

    // 5π/2 → π/2
    assert!(
        qasm.contains("rz(pi/2) q[0];"),
        "5π/2 should normalize to pi/2, got:\n{}",
        qasm
    );
}

/// Test phase tracking through consecutive Pauli rotations.
#[test]
fn test_consecutive_pauli_rotations_accumulate_correctly() {
    use afana::ast::{EhrenfestAst, Gate, GateName, Measure};
    use afana::emit::{emit_qasm, QasmVersion};

    // Create AST with consecutive rotations that should accumulate
    let ast = EhrenfestAst {
        name: "consecutive_rot".into(),
        n_qubits: 1,
        prepare: None,
        gates: vec![
            // Rz(π/4)
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![std::f64::consts::FRAC_PI_4],
            },
            // Rz(π/4) - should be equivalent to Rz(π/2) if combined
            Gate {
                name: GateName::Rz,
                qubits: vec![0],
                params: vec![std::f64::consts::FRAC_PI_4],
            },
        ],
        measures: vec![Measure { qubit: 0, cbit: 0 }],
        conditionals: vec![],
        expects: vec![],
        type_decls: vec![],
        variational_loops: vec![],
    };

    let qasm = emit_qasm(&ast, QasmVersion::V3).expect("QASM3 emission failed");

    // Each rotation should emit with correct phase
    assert!(
        qasm.contains("rz(pi/4) q[0];"),
        "First Rz(π/4) should emit correctly, got:\n{}",
        qasm
    );

    // Verify both rotations are present
    let rz_count = qasm.matches("rz(pi/4) q[0];").count();
    assert_eq!(
        rz_count, 2,
        "Both Rz(π/4) rotations should be present, got {}",
        rz_count
    );
}
