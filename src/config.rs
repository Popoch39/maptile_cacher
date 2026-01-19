use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub cache_dir: PathBuf,
    pub memory_cache_size: u64,
    pub disk_cache_max_bytes: u64,
    pub upstream_timeout: Duration,
    pub cache_max_age: Duration,
    pub user_agent: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            cache_dir: env::var("CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("cache")),
            memory_cache_size: env::var("MEMORY_CACHE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10_000),
            // 50GB disk cache
            disk_cache_max_bytes: env::var("DISK_CACHE_MAX_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50 * 1024 * 1024 * 1024),
            upstream_timeout: Duration::from_secs(30),
            // OSM requires minimum 7 days cache
            cache_max_age: Duration::from_secs(7 * 24 * 60 * 60),
            user_agent: env::var("USER_AGENT")
                .unwrap_or_else(|_| "maptile_cacher/0.1 (tile caching proxy)".to_string()),
        }
    }
}
