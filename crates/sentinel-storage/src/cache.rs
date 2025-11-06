//! Caching layer for baselines and hot data.

use moka::future::Cache;
use llm_sentinel_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_capacity: u64,
    /// Time to live (seconds)
    pub ttl_secs: u64,
    /// Time to idle (seconds)
    pub tti_secs: Option<u64>,
    /// Enable metrics
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            ttl_secs: 300,    // 5 minutes
            tti_secs: None,   // No idle timeout
            enable_metrics: true,
        }
    }
}

/// In-memory cache for baselines using Moka
pub struct BaselineCache<K, V>
where
    K: std::hash::Hash + Eq + Send + Sync + Clone + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache: Cache<K, V>,
    config: CacheConfig,
}

impl<K, V> BaselineCache<K, V>
where
    K: std::hash::Hash + Eq + Send + Sync + Clone + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new baseline cache
    pub fn new(config: CacheConfig) -> Self {
        info!(
            "Creating cache with capacity {} and TTL {}s",
            config.max_capacity, config.ttl_secs
        );

        let mut builder = Cache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(Duration::from_secs(config.ttl_secs));

        if let Some(tti) = config.tti_secs {
            builder = builder.time_to_idle(Duration::from_secs(tti));
        }

        let cache = builder.build();

        Self { cache, config }
    }

    /// Get a value from cache
    pub async fn get(&self, key: &K) -> Option<V> {
        let value = self.cache.get(key).await;

        if value.is_some() {
            metrics::counter!("sentinel_cache_hits_total").increment(1);
            debug!("Cache hit");
        } else {
            metrics::counter!("sentinel_cache_misses_total").increment(1);
            debug!("Cache miss");
        }

        value
    }

    /// Insert a value into cache
    pub async fn insert(&self, key: K, value: V) {
        self.cache.insert(key, value).await;
        metrics::counter!("sentinel_cache_inserts_total").increment(1);
    }

    /// Remove a value from cache
    pub async fn remove(&self, key: &K) {
        self.cache.invalidate(key).await;
        metrics::counter!("sentinel_cache_removals_total").increment(1);
    }

    /// Clear all entries
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        info!("Cache cleared");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entry_count = self.cache.entry_count();
        let weighted_size = self.cache.weighted_size();

        CacheStats {
            entry_count,
            weighted_size,
            max_capacity: self.config.max_capacity,
            ttl_secs: self.config.ttl_secs,
        }
    }

    /// Get cache hit rate (requires metrics)
    pub fn hit_rate(&self) -> f64 {
        // This would require tracking hits/misses
        // For now, return 0.0 - would be computed from metrics in production
        0.0
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of entries
    pub entry_count: u64,
    /// Weighted size
    pub weighted_size: u64,
    /// Maximum capacity
    pub max_capacity: u64,
    /// TTL in seconds
    pub ttl_secs: u64,
}

/// Redis-backed distributed cache
pub struct RedisCache {
    client: redis::Client,
    config: RedisCacheConfig,
}

/// Redis cache configuration
#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    /// Redis URL
    pub url: String,
    /// Key prefix
    pub key_prefix: String,
    /// Default TTL (seconds)
    pub ttl_secs: u64,
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            key_prefix: "sentinel:".to_string(),
            ttl_secs: 300,
        }
    }
}

impl RedisCache {
    /// Create a new Redis cache
    pub async fn new(config: RedisCacheConfig) -> Result<Self> {
        info!("Connecting to Redis at {}", config.url);

        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| Error::connection(format!("Failed to create Redis client: {}", e)))?;

        // Test connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| Error::connection(format!("Failed to connect to Redis: {}", e)))?;

        // Ping test
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| Error::connection(format!("Redis ping failed: {}", e)))?;

        info!("Connected to Redis successfully");

        Ok(Self { client, config })
    }

    /// Build full key with prefix
    fn build_key(&self, key: &str) -> String {
        format!("{}{}", self.config.key_prefix, key)
    }

    /// Get a value from Redis
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| Error::storage(format!("Failed to get Redis connection: {}", e)))?;

        let full_key = self.build_key(key);
        let value: Option<String> = redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::storage(format!("Redis GET failed: {}", e)))?;

        match value {
            Some(json) => {
                let parsed = serde_json::from_str(&json)
                    .map_err(|e| Error::storage(format!("Failed to deserialize: {}", e)))?;
                metrics::counter!("sentinel_cache_hits_total", "cache" => "redis").increment(1);
                Ok(Some(parsed))
            }
            None => {
                metrics::counter!("sentinel_cache_misses_total", "cache" => "redis").increment(1);
                Ok(None)
            }
        }
    }

    /// Set a value in Redis
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| Error::storage(format!("Failed to get Redis connection: {}", e)))?;

        let full_key = self.build_key(key);
        let json = serde_json::to_string(value)
            .map_err(|e| Error::storage(format!("Failed to serialize: {}", e)))?;

        redis::cmd("SETEX")
            .arg(&full_key)
            .arg(self.config.ttl_secs)
            .arg(&json)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::storage(format!("Redis SETEX failed: {}", e)))?;

        metrics::counter!("sentinel_cache_inserts_total", "cache" => "redis").increment(1);

        Ok(())
    }

    /// Delete a value from Redis
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| Error::storage(format!("Failed to get Redis connection: {}", e)))?;

        let full_key = self.build_key(key);
        redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::storage(format!("Redis DEL failed: {}", e)))?;

        metrics::counter!("sentinel_cache_removals_total", "cache" => "redis").increment(1);

        Ok(())
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| Error::storage(format!("Failed to get Redis connection: {}", e)))?;

        let full_key = self.build_key(key);
        let exists: bool = redis::cmd("EXISTS")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::storage(format!("Redis EXISTS failed: {}", e)))?;

        Ok(exists)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| Error::connection(format!("Failed to get Redis connection: {}", e)))?;

        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| Error::connection(format!("Redis health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_baseline_cache_creation() {
        let config = CacheConfig::default();
        let cache: BaselineCache<String, i32> = BaselineCache::new(config);
        let stats = cache.stats().await;
        assert_eq!(stats.entry_count, 0);
    }

    #[tokio::test]
    async fn test_baseline_cache_operations() {
        let config = CacheConfig {
            max_capacity: 100,
            ttl_secs: 60,
            tti_secs: None,
            enable_metrics: true,
        };

        let cache: BaselineCache<String, i32> = BaselineCache::new(config);

        // Insert
        cache.insert("key1".to_string(), 42).await;
        cache.insert("key2".to_string(), 100).await;

        // Get
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some(42));

        // Stats
        let stats = cache.stats().await;
        assert_eq!(stats.entry_count, 2);

        // Remove
        cache.remove(&"key1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);

        // Clear
        cache.clear().await;
        let stats = cache.stats().await;
        assert_eq!(stats.entry_count, 0);
    }

    #[test]
    fn test_redis_config_creation() {
        let config = RedisCacheConfig::default();
        assert_eq!(config.key_prefix, "sentinel:");
        assert_eq!(config.ttl_secs, 300);
    }

    #[tokio::test]
    async fn test_redis_cache_key_building() {
        let config = RedisCacheConfig::default();
        // Connection will fail without Redis, but we can test key building
        if let Ok(cache) = RedisCache::new(config).await {
            let key = cache.build_key("test");
            assert_eq!(key, "sentinel:test");
        }
    }
}
