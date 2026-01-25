//! Phase 2 Caching Layer
//!
//! Provides caching for:
//! - Historical reads from Ruvector
//! - Lineage lookups
//!
//! TTL: 60-120 seconds (configurable)

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};
use uuid::Uuid;

use super::ruvector::{SignalRecord, LineageDelta};

/// Default minimum TTL in seconds
pub const DEFAULT_TTL_MIN_SECS: u64 = 60;

/// Default maximum TTL in seconds
pub const DEFAULT_TTL_MAX_SECS: u64 = 120;

/// Default cache capacity
pub const DEFAULT_CACHE_CAPACITY: usize = 1000;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Minimum TTL in seconds
    pub ttl_min_secs: u64,
    /// Maximum TTL in seconds
    pub ttl_max_secs: u64,
    /// Maximum number of entries
    pub capacity: usize,
    /// Enable caching
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_min_secs: DEFAULT_TTL_MIN_SECS,
            ttl_max_secs: DEFAULT_TTL_MAX_SECS,
            capacity: DEFAULT_CACHE_CAPACITY,
            enabled: true,
        }
    }
}

impl CacheConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        let ttl_min_secs = std::env::var("CACHE_TTL_MIN_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TTL_MIN_SECS);

        let ttl_max_secs = std::env::var("CACHE_TTL_MAX_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TTL_MAX_SECS);

        let capacity = std::env::var("CACHE_CAPACITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_CACHE_CAPACITY);

        let enabled = std::env::var("CACHE_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        Self {
            ttl_min_secs,
            ttl_max_secs,
            capacity,
            enabled,
        }
    }

    /// Get TTL duration (uses average of min and max)
    pub fn ttl(&self) -> Duration {
        Duration::from_secs((self.ttl_min_secs + self.ttl_max_secs) / 2)
    }
}

/// Cached entry with expiration
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Generic TTL cache
#[derive(Debug)]
pub struct TtlCache<K, V> {
    entries: RwLock<HashMap<K, CacheEntry<V>>>,
    config: CacheConfig,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Create a new cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: RwLock::new(HashMap::with_capacity(config.capacity)),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Get an entry from the cache
    pub fn get(&self, key: &K) -> Option<V> {
        if !self.config.enabled {
            return None;
        }

        let entries = self.entries.read().unwrap();
        if let Some(entry) = entries.get(key) {
            if !entry.is_expired() {
                trace!("Cache hit");
                return Some(entry.value.clone());
            }
            trace!("Cache entry expired");
        }
        None
    }

    /// Insert an entry into the cache
    pub fn insert(&self, key: K, value: V) {
        if !self.config.enabled {
            return;
        }

        let mut entries = self.entries.write().unwrap();

        // Evict if at capacity
        if entries.len() >= self.config.capacity {
            self.evict_expired(&mut entries);

            // If still at capacity, remove oldest entry
            if entries.len() >= self.config.capacity {
                if let Some(oldest_key) = entries
                    .iter()
                    .min_by_key(|(_, e)| e.expires_at)
                    .map(|(k, _)| k.clone())
                {
                    entries.remove(&oldest_key);
                }
            }
        }

        let entry = CacheEntry::new(value, self.config.ttl());
        entries.insert(key, entry);
        trace!("Cache insert");
    }

    /// Remove an entry from the cache
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.write().unwrap();
        entries.remove(key).map(|e| e.value)
    }

    /// Clear all entries
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        entries.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Evict expired entries
    fn evict_expired(&self, entries: &mut HashMap<K, CacheEntry<V>>) {
        entries.retain(|_, e| !e.is_expired());
    }

    /// Run cleanup to remove expired entries
    pub fn cleanup(&self) {
        let mut entries = self.entries.write().unwrap();
        let before = entries.len();
        self.evict_expired(&mut entries);
        let after = entries.len();
        if before != after {
            debug!(evicted = before - after, "Cache cleanup");
        }
    }
}

/// Specialized cache for signal records
pub type SignalCache = TtlCache<Uuid, SignalRecord>;

/// Specialized cache for lineage lookups
pub type LineageCache = TtlCache<Uuid, Vec<LineageDelta>>;

/// Cache key for historical queries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HistoricalQueryKey {
    /// Query type identifier
    pub query_type: String,
    /// Service filter
    pub service: Option<String>,
    /// Model filter
    pub model: Option<String>,
    /// Time window hash
    pub time_hash: u64,
}

/// Historical query cache
pub type HistoricalCache = TtlCache<HistoricalQueryKey, Vec<SignalRecord>>;

/// Combined cache manager for Phase 2 agents
#[derive(Debug)]
pub struct Phase2Cache {
    /// Signal record cache
    pub signals: SignalCache,
    /// Lineage cache
    pub lineage: LineageCache,
    /// Historical query cache
    pub historical: HistoricalCache,
    /// Configuration
    config: CacheConfig,
}

impl Phase2Cache {
    /// Create a new Phase 2 cache manager
    pub fn new(config: CacheConfig) -> Self {
        Self {
            signals: TtlCache::new(config.clone()),
            lineage: TtlCache::new(config.clone()),
            historical: TtlCache::new(config.clone()),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Create from environment configuration
    pub fn from_env() -> Self {
        Self::new(CacheConfig::from_env())
    }

    /// Cache a signal record
    pub fn cache_signal(&self, signal: SignalRecord) {
        self.signals.insert(signal.signal_id, signal);
    }

    /// Get a cached signal
    pub fn get_signal(&self, signal_id: &Uuid) -> Option<SignalRecord> {
        self.signals.get(signal_id)
    }

    /// Cache lineage data
    pub fn cache_lineage(&self, signal_id: Uuid, deltas: Vec<LineageDelta>) {
        self.lineage.insert(signal_id, deltas);
    }

    /// Get cached lineage
    pub fn get_lineage(&self, signal_id: &Uuid) -> Option<Vec<LineageDelta>> {
        self.lineage.get(signal_id)
    }

    /// Cache historical query result
    pub fn cache_historical(&self, key: HistoricalQueryKey, signals: Vec<SignalRecord>) {
        self.historical.insert(key, signals);
    }

    /// Get cached historical query result
    pub fn get_historical(&self, key: &HistoricalQueryKey) -> Option<Vec<SignalRecord>> {
        self.historical.get(key)
    }

    /// Run cleanup on all caches
    pub fn cleanup_all(&self) {
        self.signals.cleanup();
        self.lineage.cleanup();
        self.historical.cleanup();
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        self.signals.clear();
        self.lineage.clear();
        self.historical.clear();
    }

    /// Get total cache size
    pub fn total_size(&self) -> usize {
        self.signals.len() + self.lineage.len() + self.historical.len()
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_ttl_cache_basic() {
        let cache: TtlCache<String, i32> = TtlCache::with_defaults();

        cache.insert("key1".to_string(), 42);
        assert_eq!(cache.get(&"key1".to_string()), Some(42));
        assert_eq!(cache.get(&"key2".to_string()), None);
    }

    #[test]
    fn test_cache_capacity() {
        let config = CacheConfig {
            ttl_min_secs: 60,
            ttl_max_secs: 120,
            capacity: 2,
            enabled: true,
        };
        let cache: TtlCache<i32, i32> = TtlCache::new(config);

        cache.insert(1, 100);
        cache.insert(2, 200);
        cache.insert(3, 300); // Should evict oldest

        assert!(cache.len() <= 2);
    }

    #[test]
    fn test_cache_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        let cache: TtlCache<String, i32> = TtlCache::new(config);

        cache.insert("key".to_string(), 42);
        assert_eq!(cache.get(&"key".to_string()), None);
    }

    #[test]
    fn test_phase2_cache() {
        let cache = Phase2Cache::with_defaults();

        let signal = SignalRecord {
            signal_id: Uuid::new_v4(),
            signal_type: "anomaly".to_string(),
            agent_name: "test".to_string(),
            agent_domain: "detection".to_string(),
            timestamp: Utc::now(),
            confidence: 0.9,
            payload: serde_json::json!({}),
            parent_signals: vec![],
            latency_ms: 10,
        };

        let id = signal.signal_id;
        cache.cache_signal(signal.clone());
        assert!(cache.get_signal(&id).is_some());
    }
}
