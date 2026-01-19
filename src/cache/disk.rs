use crate::config::Config;
use crate::error::Result;
use crate::types::{TileData, TileKey};
use bytes::Bytes;
use memmap2::Mmap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// Disk cache with zero-copy reads via mmap
#[derive(Clone)]
pub struct DiskCache {
    base_dir: PathBuf,
}

impl DiskCache {
    pub fn new(config: &Config) -> Result<Self> {
        fs::create_dir_all(&config.cache_dir)?;
        Ok(Self {
            base_dir: config.cache_dir.clone(),
        })
    }

    fn tile_path(&self, key: &TileKey) -> PathBuf {
        self.base_dir.join(key.to_path())
    }

    fn etag_path(&self, key: &TileKey) -> PathBuf {
        self.base_dir
            .join(format!("{}/{}/{}.etag", key.z, key.x, key.y))
    }

    /// Get tile from disk using mmap for zero-copy
    pub fn get(&self, key: &TileKey) -> Option<Arc<TileData>> {
        let path = self.tile_path(key);
        let file = File::open(&path).ok()?;

        // Use mmap for zero-copy read
        let mmap = unsafe { Mmap::map(&file).ok()? };
        let data = Bytes::copy_from_slice(&mmap);

        // Try to read etag
        let etag = fs::read_to_string(self.etag_path(key)).ok();

        Some(Arc::new(TileData::new(data, etag)))
    }

    /// Store tile to disk
    pub fn store(&self, key: &TileKey, data: &Bytes, etag: Option<&str>) -> Result<()> {
        let path = self.tile_path(key);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write tile data atomically
        let tmp_path = path.with_extension("tmp");
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        fs::rename(&tmp_path, &path)?;

        // Store etag if present
        if let Some(etag) = etag {
            let etag_path = self.etag_path(key);
            fs::write(etag_path, etag)?;
        }

        Ok(())
    }

    /// Get stored etag for conditional requests
    pub fn get_etag(&self, key: &TileKey) -> Option<String> {
        fs::read_to_string(self.etag_path(key)).ok()
    }

    /// Check if tile exists on disk
    pub fn exists(&self, key: &TileKey) -> bool {
        self.tile_path(key).exists()
    }
}
