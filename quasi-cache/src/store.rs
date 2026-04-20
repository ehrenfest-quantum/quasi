use crate::entry::CacheEntry;
use crate::key::CacheKey;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use thiserror::Error;
use tracing::{debug, trace, warn};

/// Errors from cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub evictions: u64,
    /// Total lookup time in microseconds.
    pub total_lookup_time_us: u64,
    /// Number of lookups performed.
    pub lookup_count: u64,
}

impl CacheStats {
    /// Compute hit rate as a ratio (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Compute average lookup time in microseconds.
    pub fn avg_lookup_time_us(&self) -> f64 {
        if self.lookup_count == 0 {
            0.0
        } else {
            self.total_lookup_time_us as f64 / self.lookup_count as f64
        }
    }

    /// Memory usage estimate in bytes (entries * average entry size).
    pub fn estimated_memory_bytes(&self) -> usize {
        // Rough estimate: ~1KB per entry on average
        self.entries * 1024
    }
}

/// Configuration for the cache store.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum entries in L1 (in-memory). 0 = unlimited.
    pub max_l1_entries: usize,
    /// Directory for L2 persistent cache. None = no persistence.
    pub l2_dir: Option<PathBuf>,
    /// Maximum age in seconds before an entry is considered stale.
    pub max_age_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_l1_entries: 10_000,
            l2_dir: None,
            max_age_seconds: 14400, // 4 hours (typical calibration window)
        }
    }
}

/// Content-addressed cache for quantum computation results.
///
/// Two-level architecture:
/// - L1: In-memory HashMap (fast, bounded, volatile)
/// - L2: Filesystem JSON files (persistent, unbounded, survives restarts)
pub struct CacheStore {
    l1: RwLock<HashMap<CacheKey, CacheEntry>>,
    config: CacheConfig,
    stats: RwLock<CacheStats>,
}

impl CacheStore {
    /// Create a new cache store with the given configuration.
    pub fn new(config: CacheConfig) -> Self {
        debug!(
            max_l1 = config.max_l1_entries,
            l2 = ?config.l2_dir,
            max_age = config.max_age_seconds,
            "creating cache store"
        );
        Self {
            l1: RwLock::new(HashMap::new()),
            config,
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Look up a cached result.
    ///
    /// Checks L1 first, then L2. Promotes L2 hits to L1.
    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        // Check L1
        {
            let l1 = self.l1.read().expect("L1 lock poisoned");
            if let Some(entry) = l1.get(key) {
                trace!(%key, "L1 cache hit");
                let mut stats = self.stats.write().expect("stats lock poisoned");
                stats.hits += 1;
                return Some(entry.clone());
            }
        }

        // Check L2
        if let Some(entry) = self.l2_read(key) {
            trace!(%key, "L2 cache hit, promoting to L1");
            let mut stats = self.stats.write().expect("stats lock poisoned");
            stats.hits += 1;
            drop(stats);
            // Promote to L1
            self.l1_insert(entry.clone());
            return Some(entry);
        }

        // Miss
        trace!(%key, "cache miss");
        let mut stats = self.stats.write().expect("stats lock poisoned");
        stats.misses += 1;
        None
    }

    /// Store a result in the cache (write-through: L1 + L2).
    pub fn put(&self, entry: CacheEntry) {
        debug!(%entry.key, backend = %entry.backend_id, "caching result");
        self.l2_write(&entry);
        self.l1_insert(entry);
    }

    /// Check if a key exists without retrieving the full entry.
    pub fn contains(&self, key: &CacheKey) -> bool {
        {
            let l1 = self.l1.read().expect("L1 lock poisoned");
            if l1.contains_key(key) {
                return true;
            }
        }
        self.l2_path(key)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let mut stats = self.stats.read().expect("stats lock poisoned").clone();
        let l1 = self.l1.read().expect("L1 lock poisoned");
        stats.entries = l1.len();
        stats
    }

    /// Remove stale entries from L1. Returns count of evicted entries.
    pub fn evict_stale(&self, now: u64) -> usize {
        let mut l1 = self.l1.write().expect("L1 lock poisoned");
        let before = l1.len();
        l1.retain(|_, entry| !entry.is_stale(self.config.max_age_seconds, now));
        let evicted = before - l1.len();
        if evicted > 0 {
            debug!(evicted, "evicted stale entries from L1");
            let mut stats = self.stats.write().expect("stats lock poisoned");
            stats.evictions += evicted as u64;
        }
        evicted
    }

    /// Remove all entries from L1.
    pub fn clear(&self) {
        let mut l1 = self.l1.write().expect("L1 lock poisoned");
        l1.clear();
        debug!("L1 cache cleared");
    }

    /// Insert into L1, evicting oldest entry if over capacity.
    fn l1_insert(&self, entry: CacheEntry) {
        let mut l1 = self.l1.write().expect("L1 lock poisoned");
        if self.config.max_l1_entries > 0 && l1.len() >= self.config.max_l1_entries {
            // Evict the oldest entry (lowest timestamp)
            if let Some(oldest_key) = l1
                .iter()
                .min_by_key(|(_, e)| e.timestamp)
                .map(|(k, _)| *k)
            {
                l1.remove(&oldest_key);
                let mut stats = self.stats.write().expect("stats lock poisoned");
                stats.evictions += 1;
                trace!(%oldest_key, "evicted oldest L1 entry for capacity");
            }
        }
        l1.insert(entry.key, entry);
    }

    fn l2_path(&self, key: &CacheKey) -> Option<PathBuf> {
        self.config
            .l2_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.json", key)))
    }

    fn l2_read(&self, key: &CacheKey) -> Option<CacheEntry> {
        let path = self.l2_path(key)?;
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(?path, %e, "failed to read L2 cache file");
                }
                return None;
            }
        };
        match serde_json::from_str(&data) {
            Ok(entry) => Some(entry),
            Err(e) => {
                warn!(?path, %e, "failed to deserialize L2 cache entry");
                None
            }
        }
    }

    fn l2_write(&self, entry: &CacheEntry) {
        if let Some(path) = self.l2_path(&entry.key) {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!(?parent, %e, "failed to create L2 cache directory");
                    return;
                }
            }
            match serde_json::to_string_pretty(entry) {
                Ok(data) => {
                    if let Err(e) = std::fs::write(&path, data) {
                        warn!(?path, %e, "failed to write L2 cache file");
                    }
                }
                Err(e) => {
                    warn!(%e, "failed to serialize cache entry");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::CacheKey;
    use std::collections::{BTreeMap, HashMap};

    fn make_entry(key: CacheKey, timestamp: u64) -> CacheEntry {
        let mut counts = HashMap::new();
        counts.insert("00".to_string(), 512);
        counts.insert("11".to_string(), 512);
        CacheEntry {
            key,
            backend_id: "test_backend".to_string(),
            calibration_version: "v1".to_string(),
            parameters: BTreeMap::new(),
            measurement_counts: counts,
            expectation_values: None,
            fidelity_estimate: Some(0.95),
            shot_count: 1024,
            timestamp,
            attestation_ref: None,
        }
    }

    #[test]
    fn put_then_get_returns_entry() {
        let store = CacheStore::new(CacheConfig::default());
        let key = CacheKey::circuit_only(b"bell_pair");
        let entry = make_entry(key, 1000);

        store.put(entry.clone());
        let got = store.get(&key).expect("should find entry");
        assert_eq!(got.key, key);
        assert_eq!(got.shot_count, 1024);
        assert_eq!(got.fidelity_estimate, Some(0.95));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let store = CacheStore::new(CacheConfig::default());
        let key = CacheKey::circuit_only(b"nonexistent");
        assert!(store.get(&key).is_none());
    }

    #[test]
    fn contains_works() {
        let store = CacheStore::new(CacheConfig::default());
        let key = CacheKey::circuit_only(b"grover");
        assert!(!store.contains(&key));

        store.put(make_entry(key, 1000));
        assert!(store.contains(&key));
    }

    #[test]
    fn evict_stale_removes_old_keeps_fresh() {
        let config = CacheConfig {
            max_age_seconds: 100,
            ..CacheConfig::default()
        };
        let store = CacheStore::new(config);

        let old_key = CacheKey::circuit_only(b"old");
        let fresh_key = CacheKey::circuit_only(b"fresh");
        store.put(make_entry(old_key, 100)); // age=200 at now=300
        store.put(make_entry(fresh_key, 250)); // age=50 at now=300

        let evicted = store.evict_stale(300);
        assert_eq!(evicted, 1);
        assert!(store.get(&old_key).is_none());
        assert!(store.get(&fresh_key).is_some());
    }

    #[test]
    fn l1_capacity_triggers_eviction() {
        let config = CacheConfig {
            max_l1_entries: 2,
            l2_dir: None,
            max_age_seconds: 14400,
        };
        let store = CacheStore::new(config);

        let k1 = CacheKey::circuit_only(b"first");
        let k2 = CacheKey::circuit_only(b"second");
        let k3 = CacheKey::circuit_only(b"third");

        store.put(make_entry(k1, 100)); // oldest
        store.put(make_entry(k2, 200));
        store.put(make_entry(k3, 300)); // this should evict k1

        // k1 should have been evicted (oldest timestamp)
        assert!(store.get(&k1).is_none());
        assert!(store.get(&k2).is_some());
        assert!(store.get(&k3).is_some());

        let stats = store.stats();
        assert!(stats.evictions >= 1);
    }

    #[test]
    fn l2_persistence_across_stores() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let l2_dir = tmp.path().to_path_buf();

        let key = CacheKey::circuit_only(b"persistent");

        // Store 1: put entry
        {
            let config = CacheConfig {
                l2_dir: Some(l2_dir.clone()),
                ..CacheConfig::default()
            };
            let store = CacheStore::new(config);
            store.put(make_entry(key, 1000));
        }

        // Store 2: new store from same dir, should find entry via L2
        {
            let config = CacheConfig {
                l2_dir: Some(l2_dir),
                ..CacheConfig::default()
            };
            let store = CacheStore::new(config);
            let got = store.get(&key).expect("should find entry from L2");
            assert_eq!(got.key, key);
            assert_eq!(got.timestamp, 1000);
        }
    }

    #[test]
    fn stats_tracking() {
        let store = CacheStore::new(CacheConfig::default());
        let key = CacheKey::circuit_only(b"stats_test");

        // Miss
        store.get(&key);
        let s = store.stats();
        assert_eq!(s.misses, 1);
        assert_eq!(s.hits, 0);

        // Put + hit
        store.put(make_entry(key, 1000));
        store.get(&key);
        let s = store.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
        assert_eq!(s.entries, 1);
    }

    #[test]
    fn clear_removes_all_l1_entries() {
        let store = CacheStore::new(CacheConfig::default());
        let k1 = CacheKey::circuit_only(b"a");
        let k2 = CacheKey::circuit_only(b"b");
        store.put(make_entry(k1, 100));
        store.put(make_entry(k2, 200));

        assert_eq!(store.stats().entries, 2);
        store.clear();
        assert_eq!(store.stats().entries, 0);
        assert!(store.get(&k1).is_none());
    }
}
