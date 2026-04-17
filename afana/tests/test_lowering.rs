use afana::cbor::{EhrenfestProgram, Hamiltonian, PauliOp, PauliOpEntry, PauliTerm, EvolutionTime, NoiseConstraint, Observable, SystemDef};
use afana::trotter::{trotterize, TrotterOrder};
use afana::lower::lower_ast_to_zx;
use afana::zx_ir::SpiderColor;

#[test]
fn test_lowering_with_coefficient() {
    // Build a program with one Pauli term: 0.5 * Z0
    let program = EhrenfestProgram {
        version: 1,
        system: SystemDef {
            n_qubits: 1,
            cooling_profile: None,
            backend_hint: None,
        },
        hamiltonian: Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 0.5,
                paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
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

    // Trotterize to get the AST
    let ast = trotterize(&program, TrotterOrder::First);

    // Lower to ZX
    let zx = lower_ast_to_zx(&ast).unwrap();

    // The circuit should have: input, Rz gate (which becomes a Z spider), output.
    // The Rz gate has angle = 2 * 0.5 * 1.0 = 1.0 radian.
    // In the ZX graph, the Rz gate becomes a Z spider with phase = angle / pi = 1.0 / std::f64::consts::PI.
    // We expect one spider between input and output with that phase.

    // The graph should have 3 spiders: input, gate, output.
    assert_eq!(zx.spider_count(), 3);

    // The middle spider (index 1) should be a Z spider with phase 1.0 / PI.
    let spider = zx.spider(1);
    assert_eq!(spider.color, SpiderColor::Z);
    assert!((spider.phase - (1.0 / std::f64::consts::PI)).abs() < 1e-10);
}