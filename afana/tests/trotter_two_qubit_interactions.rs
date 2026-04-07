// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors

use afana::ast::*;
use afana::cbor::*;
use afana::emit::{emit_qasm, QasmVersion};
use afana::trotter::{trotterize, TrotterOrder};

#[test]
fn test_xx_interaction_qasm3() {
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 0.5,
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }, PauliOpEntry { qubit: 1, axis: PauliOp::X }],
            }],
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

    let ast = trotterize(&program, TrotterOrder::First);
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    // Check for expected QASM3 pattern: H q[0]; cx q[0], q[1]; rz(1.0) q[1]; cx q[0], q[1]; h q[0];
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("cx q[0], q[1];"));
    assert!(qasm.contains("rz(1.0) q[1];"));
}

#[test]
fn test_yy_interaction_qasm3() {
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 0.5,
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Y }, PauliOpEntry { qubit: 1, axis: PauliOp::Y }],
            }],
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

    let ast = trotterize(&program, TrotterOrder::First);
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    // Check for expected QASM3 pattern for YY interaction
    assert!(qasm.contains("sdg q[0];"));
    assert!(qasm.contains("sdg q[1];"));
    assert!(qasm.contains("h q[0];"));
    assert!(qasm.contains("h q[1];"));
    assert!(qasm.contains("cx q[0], q[1];"));
    assert!(qasm.contains("rz(1.0) q[1];"));
    assert!(qasm.contains("s q[0];"));
    assert!(qasm.contains("s q[1];"));
}

#[test]
fn test_zz_interaction_qasm3() {
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 2,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 0.5,
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }, PauliOpEntry { qubit: 1, axis: PauliOp::Z }],
            }],
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

    let ast = trotterize(&program, TrotterOrder::First);
    let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
    
    // Check for expected QASM3 pattern: cx q[0], q[1]; rz(1.0) q[1]; cx q[0], q[1];
    assert!(qasm.contains("cx q[0], q[1];"));
    assert!(qasm.contains("rz(1.0) q[1];"));
}
