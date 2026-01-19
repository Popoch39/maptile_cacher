use crate::types::TileKey;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Notify;

/// Request coalescing to deduplicate concurrent requests for the same tile
pub struct RequestCoalescer {
    in_flight: DashMap<TileKey, Arc<Notify>>,
}

impl RequestCoalescer {
    pub fn new() -> Self {
        Self {
            in_flight: DashMap::new(),
        }
    }

    /// Try to acquire a lock for fetching a tile.
    /// Returns Ok(guard) if this is the first request for this tile.
    /// Returns Err(notify) if another request is already fetching this tile.
    pub fn try_acquire(&self, key: TileKey) -> CoalesceResult<'_> {
        let notify = Arc::new(Notify::new());

        match self.in_flight.entry(key) {
            dashmap::Entry::Occupied(entry) => {
                CoalesceResult::Wait(entry.get().clone())
            }
            dashmap::Entry::Vacant(entry) => {
                entry.insert(notify);
                CoalesceResult::Acquired(CoalesceGuard {
                    key,
                    in_flight: &self.in_flight,
                })
            }
        }
    }
}

pub enum CoalesceResult<'a> {
    Acquired(CoalesceGuard<'a>),
    Wait(Arc<Notify>),
}

pub struct CoalesceGuard<'a> {
    key: TileKey,
    in_flight: &'a DashMap<TileKey, Arc<Notify>>,
}

impl<'a> CoalesceGuard<'a> {
    pub fn complete(self) {
        if let Some((_, notify)) = self.in_flight.remove(&self.key) {
            notify.notify_waiters();
        }
    }
}

impl<'a> Drop for CoalesceGuard<'a> {
    fn drop(&mut self) {
        if let Some((_, notify)) = self.in_flight.remove(&self.key) {
            notify.notify_waiters();
        }
    }
}

impl Default for RequestCoalescer {
    fn default() -> Self {
        Self::new()
    }
}
