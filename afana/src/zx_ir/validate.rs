//! ZX-IR validation for Pauli-term coefficient sign consistency.
//!
//! Ensures that Pauli-term coefficients maintain consistent sign conventions
//! across Trotter steps to prevent phase errors in time-evolution simulations.

use crate::zx_ir::{ZxGraph, ZxValidationError};

/// Validate that all Pauli-term coefficients in a ZX-IR graph have consistent sign conventions.
///
/// This prevents phase errors in time-evolution simulations by ensuring that
/// Hamiltonian terms with negative coefficients are properly tracked through
/// ZX-graph transformations before QASM3 emission.
///
/// Returns `Ok(())` if all coefficients have consistent signs, or an error
/// if mixed signs are detected.
pub fn validate_sign_consistency(graph: &ZxGraph) -> Result<(), Vec<ZxValidationError>> {
    let mut errors = Vec::new();
    
    // Track the sign of the first non-zero coefficient encountered
    let mut expected_sign: Option<f64> = None;
    
    for (i, spider) in graph.spiders.iter().enumerate() {
        // Only consider Z-spiders with non-zero phase (representing Pauli terms)
        if spider.color == crate::zx_ir::SpiderColor::Z && spider.phase != 0.0 {
            let current_sign = spider.phase.signum();
            
            match expected_sign {
                None => {
                    expected_sign = Some(current_sign);
                }
                Some(sign) => {
                    if sign != current_sign {
                        errors.push(ZxValidationError::StructuralError(
                            format!("mixed coefficient signs detected: expected {}, found {} at node {}", 
                                   if sign > 0.0 { "positive" } else { "negative" },
                                   if current_sign > 0.0 { "positive" } else { "negative" },
                                   i)
                        ));
                    }
                }
            }
        }
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zx_ir::{SpiderColor, ZxGraph};
    
    #[test]
    fn test_consistent_positive_signs() {
        let mut graph = ZxGraph::new();
        // Add Z-spiders with positive phases
        graph.add_spider(SpiderColor::Z, 0.5, Some(0));
        graph.add_spider(SpiderColor::Z, 1.0, Some(1));
        graph.add_spider(SpiderColor::Z, 0.25, Some(2));
        
        assert!(validate_sign_consistency(&graph).is_ok());
    }
    
    #[test]
    fn test_consistent_negative_signs() {
        let mut graph = ZxGraph::new();
        // Add Z-spiders with negative phases
        graph.add_spider(SpiderColor::Z, -0.5, Some(0));
        graph.add_spider(SpiderColor::Z, -1.0, Some(1));
        graph.add_spider(SpiderColor::Z, -0.25, Some(2));
        
        assert!(validate_sign_consistency(&graph).is_ok());
    }
    
    #[test]
    fn test_mixed_signs_rejected() {
        let mut graph = ZxGraph::new();
        // Add Z-spiders with mixed signs
        graph.add_spider(SpiderColor::Z, 0.5, Some(0));
        graph.add_spider(SpiderColor::Z, -1.0, Some(1)); // Mixed sign
        
        let result = validate_sign_consistency(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors[0].to_string().contains("mixed coefficient signs"));
    }
    
    #[test]
    fn test_zero_phase_ignored() {
        let mut graph = ZxGraph::new();
        // Zero phase should be ignored in sign consistency check
        graph.add_spider(SpiderColor::Z, 0.0, Some(0));
        graph.add_spider(SpiderColor::Z, 0.5, Some(1));
        graph.add_spider(SpiderColor::Z, 1.0, Some(2));
        
        assert!(validate_sign_consistency(&graph).is_ok());
    }
    
    #[test]
    fn test_x_spiders_ignored() {
        let mut graph = ZxGraph::new();
        // X-spiders should be ignored in sign consistency check
        graph.add_spider(SpiderColor::X, 0.5, Some(0));
        graph.add_spider(SpiderColor::Z, 0.5, Some(1));
        graph.add_spider(SpiderColor::Z, 1.0, Some(2));
        
        assert!(validate_sign_consistency(&graph).is_ok());
    }
    
    #[test]
    fn test_empty_graph() {
        let graph = ZxGraph::new();
        // Empty graph should pass (no signs to be inconsistent)
        assert!(validate_sign_consistency(&graph).is_ok());
    }
}