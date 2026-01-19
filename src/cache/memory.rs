use crate::types::{TileData, TileKey};
use bytes::Bytes;
use moka::future::Cache;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryCache {
    cache: Cache<TileKey, Arc<TileData>>,
}

impl MemoryCache {
    pub fn new(max_capacity: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .weigher(|_key: &TileKey, value: &Arc<TileData>| -> u32 {
                let size = value.data.len() + value.etag.as_ref().map_or(0, |e| e.len()) + 64;
                size.min(u32::MAX as usize) as u32
            })
            .build();

        Self { cache }
    }

    pub async fn get(&self, key: &TileKey) -> Option<Arc<TileData>> {
        self.cache.get(key).await
    }

    pub async fn insert(&self, key: TileKey, data: Bytes, etag: Option<String>) {
        let tile_data = Arc::new(TileData::new(data, etag));
        self.cache.insert(key, tile_data).await;
    }

    pub async fn insert_tile(&self, key: TileKey, tile: Arc<TileData>) {
        self.cache.insert(key, tile).await;
    }

    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }
}
