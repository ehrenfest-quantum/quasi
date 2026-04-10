use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A 32-byte content-addressed cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Build a cache key from circuit content + parameters + backend + calibration.
    ///
    /// The key is deterministic: same inputs always produce the same key.
    /// Calibration version is part of the key (Nix model): changed calibration
    /// = different key = no invalidation needed.
    pub fn build(
        circuit_bytes: &[u8],
        backend_id: &str,
        parameters: &BTreeMap<String, f64>,
        calibration_version: &str,
    ) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(circuit_bytes);
        hasher.update(backend_id.as_bytes());
        // Parameters: sorted by BTreeMap, serialize deterministically
        for (k, v) in parameters {
            hasher.update(k.as_bytes());
            hasher.update(&v.to_le_bytes());
        }
        hasher.update(calibration_version.as_bytes());
        CacheKey(*hasher.finalize().as_bytes())
    }

    /// Build a key from just circuit bytes (for lookup without backend/calibration).
    pub fn circuit_only(circuit_bytes: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"circuit:");
        hasher.update(circuit_bytes);
        CacheKey(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encode the full 32-byte key as a 64-character hex string.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for byte in &self.0 {
            s.push(HEX_CHARS[(byte >> 4) as usize]);
            s.push(HEX_CHARS[(byte & 0x0f) as usize]);
        }
        s
    }
}

const HEX_CHARS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display first 16 hex chars for readability
        for byte in &self.0[..8] {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_produce_same_key() {
        let mut params = BTreeMap::new();
        params.insert("theta".to_string(), 1.57);
        let k1 = CacheKey::build(b"OPENQASM 2.0;", "ibm_nairobi", &params, "cal_v3");
        let k2 = CacheKey::build(b"OPENQASM 2.0;", "ibm_nairobi", &params, "cal_v3");
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_inputs_produce_different_key() {
        let params = BTreeMap::new();
        let k1 = CacheKey::build(b"circuit_a", "backend_a", &params, "cal_v1");
        let k2 = CacheKey::build(b"circuit_b", "backend_a", &params, "cal_v1");
        assert_ne!(k1, k2);

        let k3 = CacheKey::build(b"circuit_a", "backend_b", &params, "cal_v1");
        assert_ne!(k1, k3);

        let k4 = CacheKey::build(b"circuit_a", "backend_a", &params, "cal_v2");
        assert_ne!(k1, k4);
    }

    #[test]
    fn parameter_order_does_not_matter() {
        // BTreeMap sorts by key, so insertion order is irrelevant
        let mut params1 = BTreeMap::new();
        params1.insert("alpha".to_string(), 1.0);
        params1.insert("beta".to_string(), 2.0);

        let mut params2 = BTreeMap::new();
        params2.insert("beta".to_string(), 2.0);
        params2.insert("alpha".to_string(), 1.0);

        let k1 = CacheKey::build(b"circ", "be", &params1, "cv");
        let k2 = CacheKey::build(b"circ", "be", &params2, "cv");
        assert_eq!(k1, k2);
    }

    #[test]
    fn display_shows_hex_prefix() {
        let key = CacheKey::circuit_only(b"hello");
        let display = format!("{}", key);
        assert_eq!(display.len(), 16); // 8 bytes * 2 hex chars
        assert!(display.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn to_hex_returns_full_64_chars() {
        let key = CacheKey::circuit_only(b"test");
        let hex = key.to_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn circuit_only_differs_from_build() {
        let params = BTreeMap::new();
        let k1 = CacheKey::circuit_only(b"circ");
        let k2 = CacheKey::build(b"circ", "", &params, "");
        // circuit_only uses a "circuit:" domain separator, so these should differ
        assert_ne!(k1, k2);
    }
}
