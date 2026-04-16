use afana::zx_ir::{Graph, SpiderColor, ZxValidationError};

#[test]
fn test_validate_rejects_large_coefficient() {
    let mut graph = Graph::new();
    let spider = graph.add_spider(SpiderColor::Z, 15.0 * std::f64::consts::PI, Some(0));
    graph.set_inputs(vec![spider]);
    graph.set_outputs(vec![spider]);
    
    let result = graph.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(e, ZxValidationError::CoefficientOutOfRange { coefficient: _, qubit: 0 })));
}

#[test]
fn test_validate_accepts_valid_coefficient() {
    let mut graph = Graph::new();
    let spider = graph.add_spider(SpiderColor::Z, 3.0 * std::f64::consts::PI, Some(0));
    graph.set_inputs(vec![spider]);
    graph.set_outputs(vec![spider]);
    
    let result = graph.validate();
    assert!(result.is_ok());
}