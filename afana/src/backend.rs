use serde::{Deserialize, Serialize};

/// Represents the result of a quantum job from a backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// The bitstrings returned by the quantum backend.
    pub counts: Vec<String>,
    /// The number of shots (executions) for this job.
    pub shots: u32,
}

/// Parse a raw JSON response from a quantum backend into a common JobResult structure.
///
/// Supports both IBM Quantum and IQM response formats.
pub fn parse_result(json: &str, expected_qubits: usize) -> Result<JobResult, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct IBMResponse {
        #[serde(rename = "results")]
        results: Vec<IBMResult>,
    }

    #[derive(Deserialize)]
    struct IBMResult {
        #[serde(rename = "data")]
        data: IBMData,
    }

    #[derive(Deserialize)]
    struct IBMData {
        #[serde(rename = "counts")]
        counts: std::collections::HashMap<String, u32>,
    }

    #[derive(Deserialize)]
    struct IQMResponse {
        #[serde(rename = "shots")]
        shots: u32,
        #[serde(rename = "counts")]
        counts: std::collections::HashMap<String, u32>,
    }

    // Try to parse as IBM Quantum response first
    if let Ok(ibm_resp) = serde_json::from_str::<IBMResponse>(json) {
        let counts: Vec<String> = ibm_resp
            .results
            .first()
            .map(|r| r.data.counts.keys().cloned().collect())
            .unwrap_or_default();
        
        // Validate that all bitstrings have the expected qubit count
        for bitstring in &counts {
            if bitstring.len() != expected_qubits {
                return Err(format!("Bitstring '{}' has {} bits, expected {}", bitstring, bitstring.len(), expected_qubits).into());
            }
        }
        
        return Ok(JobResult {
            counts,
            shots: ibm_resp.results.first().map(|r| r.data.counts.values().sum()).unwrap_or(0),
        });
    }

    // Try to parse as IQM response
    if let Ok(iqm_resp) = serde_json::from_str::<IQMResponse>(json) {
        let counts: Vec<String> = iqm_resp.counts.keys().cloned().collect();
        
        // Validate that all bitstrings have the expected qubit count
        for bitstring in &counts {
            if bitstring.len() != expected_qubits {
                return Err(format!("Bitstring '{}' has {} bits, expected {}", bitstring, bitstring.len(), expected_qubits).into());
            }
        }
        
        return Ok(JobResult {
            counts,
            shots: iqm_resp.shots,
        });
    }

    Err("Failed to parse response from any known backend format".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ibm_response() {
        let mock_ibm_response = r#'{
            "results": [
                {
                    "data": {
                        "counts": {
                            "00": 128,
                            "01": 130,
                            "10": 125,
                            "11": 117
                        }
                    }
                }
            ]
        }'#;
        
        let result = parse_result(mock_ibm_response, 2).unwrap();
        
        assert_eq!(result.counts.len(), 4);
        assert_eq!(result.shots, 500);
        
        // Verify all bitstrings have correct length
        for bitstring in &result.counts {
            assert_eq!(bitstring.len(), 2);
        }
        
        // Verify specific bitstrings
        assert!(result.counts.contains(&"00".to_string()));
        assert!(result.counts.contains(&"01".to_string()));
        assert!(result.counts.contains(&"10".to_string()));
        assert!(result.counts.contains(&"11".to_string()));
    }

    #[test]
    fn test_parse_iqm_response() {
        let mock_iqm_response = r#'{
            "shots": 1000,
            "counts": {
                "000": 125,
                "001": 120,
                "010": 130,
                "011": 128,
                "100": 122,
                "101": 127,
                "110": 131,
                "111": 137
            }
        }'#;
        
        let result = parse_result(mock_iqm_response, 3).unwrap();
        
        assert_eq!(result.counts.len(), 8);
        assert_eq!(result.shots, 1000);
        
        // Verify all bitstrings have correct length
        for bitstring in &result.counts {
            assert_eq!(bitstring.len(), 3);
        }
    }

    #[test]
    fn test_bitstring_validation() {
        let mock_ibm_response = r#'{
            "results": [
                {
                    "data": {
                        "counts": {
                            "00": 128,
                            "01": 130,
                            "10": 125,
                            "11": 117
                        }
                    }
                }
            ]
        }'#;
        
        // This should fail because we expect 3 qubits but get 2-bit strings
        let result = parse_result(mock_ibm_response, 3);
        assert!(result.is_err());
    }
}