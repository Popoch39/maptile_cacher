use crate::cache::coalescing::CoalesceResult;
use crate::cache::{DiskCache, MemoryCache, RequestCoalescer};
use crate::error::{AppError, Result};
use crate::types::TileKey;
use crate::upstream::{FetchResult, OsmFetcher};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

pub struct AppState {
    pub memory_cache: MemoryCache,
    pub disk_cache: DiskCache,
    pub coalescer: RequestCoalescer,
    pub fetcher: OsmFetcher,
    pub cache_max_age_secs: u64,
}

pub async fn get_tile(
    State(state): State<Arc<AppState>>,
    Path((z, x, filename)): Path<(u8, u32, String)>,
    headers: HeaderMap,
) -> Result<Response> {
    // Parse y from filename (e.g., "5461.png" -> 5461)
    let y: u32 = filename
        .strip_suffix(".png")
        .ok_or(AppError::InvalidCoordinates)?
        .parse()
        .map_err(|_| AppError::InvalidCoordinates)?;

    let key = TileKey::new(z, x, y);

    // Validate coordinates
    let max_coord = 1u32 << z;
    if x >= max_coord || y >= max_coord {
        return Err(AppError::InvalidCoordinates);
    }

    // Check client's If-None-Match
    let client_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok());

    // 1. Check memory cache
    if let Some(tile) = state.memory_cache.get(&key).await {
        tracing::trace!(key = %key, "Memory cache hit");
        return make_response(&tile.data, tile.etag.as_deref(), client_etag, state.cache_max_age_secs);
    }

    // 2. Check disk cache
    if let Some(tile) = state.disk_cache.get(&key) {
        tracing::trace!(key = %key, "Disk cache hit");
        // Promote to memory cache
        state.memory_cache.insert_tile(key, tile.clone()).await;
        return make_response(&tile.data, tile.etag.as_deref(), client_etag, state.cache_max_age_secs);
    }

    // 3. Fetch from upstream with request coalescing
    let tile = fetch_with_coalescing(&state, key).await?;

    make_response(&tile.data, tile.etag.as_deref(), client_etag, state.cache_max_age_secs)
}

async fn fetch_with_coalescing(
    state: &Arc<AppState>,
    key: TileKey,
) -> Result<Arc<crate::types::TileData>> {
    loop {
        match state.coalescer.try_acquire(key) {
            CoalesceResult::Acquired(guard) => {
                // We're responsible for fetching
                let stored_etag = state.disk_cache.get_etag(&key);

                let result = state.fetcher.fetch(&key, stored_etag.as_deref()).await;

                // Complete guard before processing result to unblock waiters
                guard.complete();

                match result {
                    Ok(FetchResult::Data(tile)) => {
                        let data = tile.data.clone();
                        let etag = tile.etag.clone();

                        // Store to caches
                        if let Err(e) = state.disk_cache.store(&key, &data, etag.as_deref()) {
                            tracing::warn!(key = %key, error = %e, "Failed to store to disk cache");
                        }
                        state.memory_cache.insert(key, data.clone(), etag.clone()).await;

                        return Ok(Arc::new(tile));
                    }
                    Ok(FetchResult::NotModified) => {
                        // Re-read from disk cache (should exist since we had an etag)
                        if let Some(tile) = state.disk_cache.get(&key) {
                            state.memory_cache.insert_tile(key, tile.clone()).await;
                            return Ok(tile);
                        }
                        // Fallback: fetch without etag
                        match state.fetcher.fetch(&key, None).await? {
                            FetchResult::Data(tile) => {
                                let data = tile.data.clone();
                                let etag = tile.etag.clone();
                                if let Err(e) = state.disk_cache.store(&key, &data, etag.as_deref()) {
                                    tracing::warn!(key = %key, error = %e, "Failed to store to disk cache");
                                }
                                state.memory_cache.insert(key, data, etag).await;
                                return Ok(Arc::new(tile));
                            }
                            FetchResult::NotModified => {
                                return Err(AppError::NotFound);
                            }
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
            CoalesceResult::Wait(notify) => {
                // Wait for the other request to complete
                notify.notified().await;

                // Check caches again
                if let Some(tile) = state.memory_cache.get(&key).await {
                    return Ok(tile);
                }
                if let Some(tile) = state.disk_cache.get(&key) {
                    state.memory_cache.insert_tile(key, tile.clone()).await;
                    return Ok(tile);
                }

                // Still not in cache, loop and try again
                // (this handles the case where the other request failed)
            }
        }
    }
}

fn make_response(
    data: &[u8],
    etag: Option<&str>,
    client_etag: Option<&str>,
    cache_max_age_secs: u64,
) -> Result<Response> {
    // Check if client's etag matches (304 Not Modified)
    if let (Some(server_etag), Some(client_etag)) = (etag, client_etag) {
        if server_etag == client_etag {
            return Ok(StatusCode::NOT_MODIFIED.into_response());
        }
    }

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(
            header::CACHE_CONTROL,
            format!("public, max-age={}", cache_max_age_secs),
        );

    if let Some(etag) = etag {
        builder = builder.header(header::ETAG, etag);
    }

    Ok(builder
        .body(Body::from(data.to_vec()))
        .expect("valid response"))
}
