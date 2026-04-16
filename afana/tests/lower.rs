use afana::cbor::{PauliOp, PauliOpEntry, PauliTerm};
use afana::lower::lower_single_qubit_pauli_term;
use afana::zx_ir::SpiderColor;

#[test]
fn test_single_qubit_pauli_x_produces_one_spider() {
    // X term with coefficient 0.5
    let term = PauliTerm {
        coefficient: 0.5,
        paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
    };
    let zx = lower_single_qubit_pauli_term(&term, 0);
    
    assert!(zx.validate().is_ok());
    // Should have exactly one non-boundary spider (the X spider)
    // Total: input + X spider + output = 3 spiders
    // But the issue asks for "exactly one spider node" meaning the gate spider
    // Let's verify the X spider has correct phase
    assert_eq!(zx.spider_count(), 3);
    assert_eq!(zx.spider(1).color, SpiderColor::X);
    let expected_phase = 2.0 * 0.5 / std::f64::consts::PI;
    assert!((zx.spider(1).phase - expected_phase).abs() < 1e-10);
}

#[test]
fn test_single_qubit_pauli_y_produces_correct_spiders() {
    // Y term with coefficient 0.5
    let term = PauliTerm {
        coefficient: 0.5,
        paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Y }],
    };
    let zx = lower_single_qubit_pauli_term(&term, 0);
    
    assert!(zx.validate().is_ok());
    // Y needs Z-X-Z pattern: input + Z(0.5) + X(phase) + Z(-0.5) + output = 5 spiders
    assert_eq!(zx.spider_count(), 5);
    
    // Verify the middle X spider has correct phase
    assert_eq!(zx.spider(2).color, SpiderColor::X);
    let expected_phase = 2.0 * 0.5 / std::f64::consts::PI;
    assert!((zx.spider(2).phase - expected_phase).abs() < 1e-10);
}

#[test]
fn test_single_qubit_pauli_z_produces_one_spider() {
    // Z term with coefficient 0.5
    let term = PauliTerm {
        coefficient: 0.5,
        paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
    };
    let zx = lower_single_qubit_pauli_term(&term, 0);
    
    assert!(zx.validate().is_ok());
    // Should have exactly one non-boundary spider (the Z spider)
    assert_eq!(zx.spider_count(), 3);
    assert_eq!(zx.spider(1).color, SpiderColor::Z);
    let expected_phase = 2.0 * 0.5 / std::f64::consts::PI;
    assert!((zx.spider(1).phase - expected_phase).abs() < 1e-10);
}

#[test]
fn test_single_qubit_pauli_identity_produces_no_gate_spider() {
    // Identity term (empty paulis)
    let term = PauliTerm {
        coefficient: 1.0,
        paulis: vec![],
    };
    let zx = lower_single_qubit_pauli_term(&term, 0);
    
    assert!(zx.validate().is_ok());
    // Identity produces only boundary nodes: input + output = 2 spiders
    assert_eq!(zx.spider_count(), 2);
    // Both should be Z(0) boundary spiders
    assert_eq!(zx.spider(0).color, SpiderColor::Z);
    assert_eq!(zx.spider(1).color, SpiderColor::Z);
    assert!((zx.spider(0).phase - 0.0).abs() < 1e-10);
    assert!((zx.spider(1).phase - 0.0).abs() < 1e-10);
}

#[test]
fn test_single_qubit_pauli_term_phase_attributes() {
    // Test that phase attributes are correctly computed for different coefficients
    let test_cases = [
        (0.25, 2.0 * 0.25 / std::f64::consts::PI),
        (0.5, 2.0 * 0.5 / std::f64::consts::PI),
        (1.0, 2.0 * 1.0 / std::f64::consts::PI),
        (-0.5, 2.0 * (-0.5) / std::f64::consts::PI),
    ];
    
    for (coeff, expected_phase) in test_cases {
        // X term
        let term_x = PauliTerm {
            coefficient: coeff,
            paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::X }],
        };
        let zx_x = lower_single_qubit_pauli_term(&term_x, 0);
        assert_eq!(zx_x.spider(1).color, SpiderColor::X);
        assert!((zx_x.spider(1).phase - expected_phase).abs() < 1e-10, 
            "X term with coeff {} failed: expected {}, got {}", 
            coeff, expected_phase, zx_x.spider(1).phase);
        
        // Z term
        let term_z = PauliTerm {
            coefficient: coeff,
            paulis: vec![PauliOpEntry { qubit: 0, axis: PauliOp::Z }],
        };
        let zx_z = lower_single_qubit_pauli_term(&term_z, 0);
        assert_eq!(zx_z.spider(1).color, SpiderColor::Z);
        assert!((zx_z.spider(1).phase - expected_phase).abs() < 1e-10,
            "Z term with coeff {} failed: expected {}, got {}",
            coeff, expected_phase, zx_z.spider(1).phase);
    }
}