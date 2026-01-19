use bytes::Bytes;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn new(z: u8, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    pub fn to_path(&self) -> String {
        format!("{}/{}/{}.png", self.z, self.x, self.y)
    }
}

impl Hash for TileKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(self.z);
        state.write_u32(self.x);
        state.write_u32(self.y);
    }
}

impl std::fmt::Display for TileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}

#[derive(Debug, Clone)]
pub struct TileData {
    pub data: Bytes,
    pub etag: Option<String>,
}

impl TileData {
    pub fn new(data: Bytes, etag: Option<String>) -> Self {
        Self { data, etag }
    }
}
