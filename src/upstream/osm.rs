use crate::config::Config;
use crate::error::{AppError, Result};
use crate::types::{TileData, TileKey};
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct OsmFetcher {
    client: Client,
    servers: Vec<&'static str>,
    current_server: Arc<AtomicUsize>,
}

impl OsmFetcher {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.upstream_timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .map_err(AppError::Upstream)?;

        Ok(Self {
            client,
            servers: vec![
                "a.tile.openstreetmap.org",
                "b.tile.openstreetmap.org",
                "c.tile.openstreetmap.org",
            ],
            current_server: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Get next server using round-robin
    fn next_server(&self) -> &'static str {
        let idx = self.current_server.fetch_add(1, Ordering::Relaxed) % self.servers.len();
        self.servers[idx]
    }

    fn tile_url(&self, key: &TileKey) -> String {
        let server = self.next_server();
        format!("https://{}/{}/{}/{}.png", server, key.z, key.x, key.y)
    }

    pub async fn fetch(&self, key: &TileKey, etag: Option<&str>) -> Result<FetchResult> {
        let url = self.tile_url(key);

        let mut request = self.client.get(&url);

        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }

        let response = request.send().await?;
        let status = response.status();

        match status.as_u16() {
            200 => {
                let etag = response
                    .headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let data = response.bytes().await?;
                tracing::debug!(key = %key, size = data.len(), "Fetched tile from upstream");
                Ok(FetchResult::Data(TileData::new(data, etag)))
            }
            304 => {
                tracing::debug!(key = %key, "Tile not modified (304)");
                Ok(FetchResult::NotModified)
            }
            404 => Err(AppError::NotFound),
            code => Err(AppError::UpstreamStatus(code)),
        }
    }
}

pub enum FetchResult {
    Data(TileData),
    NotModified,
}
