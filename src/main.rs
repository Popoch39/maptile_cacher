mod cache;
mod config;
mod error;
mod handlers;
mod types;
mod upstream;

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cache::{DiskCache, MemoryCache, RequestCoalescer};
use config::Config;
use handlers::{get_tile, AppState};
use upstream::OsmFetcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maptile_cacher=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::default();

    tracing::info!(bind_addr = %config.bind_addr, "Starting OSM tile caching proxy");
    tracing::info!(cache_dir = ?config.cache_dir, "Disk cache directory");
    tracing::info!(memory_cache_size = config.memory_cache_size, "Memory cache max entries");

    // Initialize components
    let memory_cache = MemoryCache::new(config.memory_cache_size);
    let disk_cache = DiskCache::new(&config)?;
    let coalescer = RequestCoalescer::new();
    let fetcher = OsmFetcher::new(&config)?;

    let state = Arc::new(AppState {
        memory_cache,
        disk_cache,
        coalescer,
        fetcher,
        cache_max_age_secs: config.cache_max_age.as_secs(),
    });

    // Build router
    let app = Router::new()
        .route("/{z}/{x}/{filename}", get(get_tile))
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("Listening on {}", config.bind_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
