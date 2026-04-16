use afana::lower::lower_single_qubit_non_pauli;
use afana::zx_ir::{SpiderColor, ZxGraph};

#[test]
fn test_lower_rotation_hamiltonian_produces_phase_shifted_spiders() {
    let mut graph = ZxGraph::new();
    let mut wires = vec![];
    
    // Set up a single qubit wire
    let inp = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    wires.push(afana::lower::WireHead { tip: inp });
    
    // Lower RZ(π/2) Hamiltonian
    lower_single_qubit_non_pauli(&mut graph, &mut wires, 0, std::f64::consts::FRAC_PI_2).unwrap();
    
    // Should produce a Z spider with phase 0.5 (π/2 / π)
    assert_eq!(graph.spider_count(), 2);
    let spider = graph.spider(1);
    assert_eq!(spider.color, SpiderColor::Z);
    assert!((spider.phase - 0.5).abs() < 1e-10);
    
    // Graph should validate
    assert!(graph.validate().is_ok());
}

#[test]
fn test_lower_rotation_passes_zx_ir_validation() {
    let mut graph = ZxGraph::new();
    let mut wires = vec![];
    
    let inp = graph.add_spider(SpiderColor::Z, 0.0, Some(0));
    wires.push(afana::lower::WireHead { tip: inp });
    
    // Test multiple rotation angles
    let angles = vec![0.0, std::f64::consts::PI, std::f64::consts::FRAC_PI_4, -std::f64::consts::FRAC_PI_3];
    
    for angle in angles {
        lower_single_qubit_non_pauli(&mut graph, &mut wires, 0, angle).unwrap();
    }
    
    // All spiders should have valid phases
    assert!(graph.validate().is_ok());
}