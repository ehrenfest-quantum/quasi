// Integration test for Heisenberg model parity validation.
// Confirms that all benchmark Heisenberg model programs pass parity validation.

use afana::cbor::*;
use afana::trotter::{trotterize, TrotterOrder};
use afana::validate::{validate_gate_parity, ParityValidationResult};

/// Build a Heisenberg model EhrenfestProgram with XX, YY, ZZ terms.
fn heisenberg_program(n_qubits: usize, steps: u32) -> EhrenfestProgram {
    let mut terms = Vec::new();

    for i in 0..n_qubits.saturating_sub(1) {
        terms.push(PauliTerm {
            coefficient: 0.5,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::X },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::X },
            ],
        });
        terms.push(PauliTerm {
            coefficient: 0.5,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Y },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Y },
            ],
        });
        terms.push(PauliTerm {
            coefficient: 0.5,
            paulis: vec![
                PauliOpEntry { qubit: i, axis: PauliOp::Z },
                PauliOpEntry { qubit: i + 1, axis: PauliOp::Z },
            ],
        });
    }

    let total_us = 1.0;
    let dt_us = total_us / steps as f64;

    EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms,
            constant_offset: 0.0,
        },
        evolution: EvolutionTime {
            total_us,
            steps,
            dt_us,
        },
        observables: vec![Observable::SZ { qubit: 0 }],
        noise: NoiseConstraint {
            t1_us: 100.0,
            t2_us: 50.0,
            gate_fidelity_min: None,
            readout_fidelity_min: None,
        },
    }
}

#[test]
fn test_heisenberg_parity() {
    // Test various Heisenberg model configurations.
    let configs = [
        (2, 1), (2, 5), (2, 10),
        (3, 1), (3, 5), (3, 10),
        (4, 1), (4, 5),
        (5, 1), (5, 2),
    ];

    for (n_qubits, steps) in configs {
        let program = heisenberg_program(n_qubits, steps);

        // First-order Trotter.
        let ast = trotterize(&program, TrotterOrder::First);
        let result = validate_gate_parity(&program, &ast.gates);
        assert_eq!(
            result, ParityValidationResult::Ok,
            "Heisenberg {n_qubits}q/{steps} steps (1st order) should pass parity validation"
        );

        // Second-order Trotter.
        let ast2 = trotterize(&program, TrotterOrder::Second);
        let result2 = validate_gate_parity(&program, &ast2.gates);
        assert_eq!(
            result2, ParityValidationResult::Ok,
            "Heisenberg {n_qubits}q/{steps} steps (2nd order) should pass parity validation"
        );
    }
}