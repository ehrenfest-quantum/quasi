use crate::key::CacheKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// A cached quantum computation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cache key that produced this entry.
    pub key: CacheKey,
    /// Backend that produced this result.
    pub backend_id: String,
    /// Calibration version at time of execution.
    pub calibration_version: String,
    /// Parameter bindings used.
    pub parameters: BTreeMap<String, f64>,
    /// Measurement counts: bitstring -> count.
    pub measurement_counts: HashMap<String, u64>,
    /// Expectation values if computed.
    pub expectation_values: Option<Vec<f64>>,
    /// Estimated fidelity of the result.
    pub fidelity_estimate: Option<f64>,
    /// Number of shots used.
    pub shot_count: u32,
    /// Unix timestamp of execution.
    pub timestamp: u64,
    /// Reference to Alsvid attestation (if QPU-attested).
    pub attestation_ref: Option<String>,
}

impl CacheEntry {
    /// Returns the age of this entry in seconds relative to `now`.
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    /// Returns true if this entry is older than `max_age_seconds`.
    pub fn is_stale(&self, max_age_seconds: u64, now: u64) -> bool {
        self.age_seconds(now) > max_age_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(timestamp: u64) -> CacheEntry {
        CacheEntry {
            key: CacheKey::circuit_only(b"test"),
            backend_id: "test_backend".to_string(),
            calibration_version: "v1".to_string(),
            parameters: BTreeMap::new(),
            measurement_counts: HashMap::new(),
            expectation_values: None,
            fidelity_estimate: None,
            shot_count: 1024,
            timestamp,
            attestation_ref: None,
        }
    }

    #[test]
    fn age_seconds_basic() {
        let entry = make_entry(100);
        assert_eq!(entry.age_seconds(150), 50);
        assert_eq!(entry.age_seconds(100), 0);
    }

    #[test]
    fn age_seconds_saturates() {
        let entry = make_entry(200);
        // now < timestamp should not underflow
        assert_eq!(entry.age_seconds(100), 0);
    }

    #[test]
    fn is_stale_works() {
        let entry = make_entry(100);
        assert!(!entry.is_stale(100, 150)); // age 50 <= 100
        assert!(entry.is_stale(100, 250)); // age 150 > 100
        assert!(!entry.is_stale(100, 200)); // age 100 == 100, not >
    }
}
