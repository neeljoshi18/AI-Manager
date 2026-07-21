//! Two-tier deduplication (Spec §3.1).
//!
//! Tier 1: Volatile cache — `SET event_id EX ttl NX` (Redis or in-memory).
//! Tier 2: Analytical engine — ClickHouse ReplacingMergeTree on event_id
//!         (handled at write / merge time in the store layer).

use crate::error::{CoreError, CoreResult};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[async_trait]
pub trait DedupStore: Send + Sync {
    /// Returns `true` if this is the first time we've seen `event_id` (should process).
    /// Returns `false` if duplicate (caller should ACK 200 and drop).
    async fn try_claim(&self, event_id: &str) -> CoreResult<bool>;

    /// Release a claim after a failed processing attempt so the event can be retried.
    async fn release(&self, event_id: &str) -> CoreResult<()>;

    /// Approximate size (for metrics / tests).
    async fn len_approx(&self) -> usize;
}

/// In-memory TTL map used for embedded mode and unit tests.
pub struct InMemoryDedup {
    ttl: Duration,
    entries: DashMap<String, Instant>,
    /// Insertion order for opportunistic GC.
    order: Mutex<VecDeque<(String, Instant)>>,
}

impl InMemoryDedup {
    pub fn new(ttl_secs: u64) -> Arc<Self> {
        Arc::new(Self {
            ttl: Duration::from_secs(ttl_secs),
            entries: DashMap::new(),
            order: Mutex::new(VecDeque::new()),
        })
    }

    fn gc(&self) {
        let now = Instant::now();
        let mut order = self.order.lock();
        while let Some((id, inserted)) = order.front() {
            if now.duration_since(*inserted) < self.ttl {
                break;
            }
            let id = id.clone();
            order.pop_front();
            if let Some(entry) = self.entries.get(&id) {
                if now.duration_since(*entry.value()) >= self.ttl {
                    drop(entry);
                    self.entries.remove(&id);
                }
            }
        }
    }
}

#[async_trait]
impl DedupStore for InMemoryDedup {
    async fn try_claim(&self, event_id: &str) -> CoreResult<bool> {
        self.gc();
        let now = Instant::now();
        match self.entries.entry(event_id.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(mut occ) => {
                if now.duration_since(*occ.get()) < self.ttl {
                    Ok(false)
                } else {
                    occ.insert(now);
                    self.order.lock().push_back((event_id.to_string(), now));
                    Ok(true)
                }
            }
            dashmap::mapref::entry::Entry::Vacant(v) => {
                v.insert(now);
                self.order.lock().push_back((event_id.to_string(), now));
                Ok(true)
            }
        }
    }

    async fn release(&self, event_id: &str) -> CoreResult<()> {
        self.entries.remove(event_id);
        Ok(())
    }

    async fn len_approx(&self) -> usize {
        self.entries.len()
    }
}

/// Helper: map false claim → Duplicate error for call sites that prefer Result.
pub async fn claim_or_duplicate(
    store: &dyn DedupStore,
    event_id: &str,
) -> CoreResult<()> {
    if store.try_claim(event_id).await? {
        Ok(())
    } else {
        Err(CoreError::Duplicate {
            event_id: event_id.to_string(),
        })
    }
}

#[cfg(feature = "production")]
pub mod redis_impl {
    use super::*;

    pub struct RedisDedup {
        conn: redis::aio::ConnectionManager,
        ttl_secs: u64,
        key_prefix: String,
    }

    impl RedisDedup {
        pub async fn connect(redis_url: &str, ttl_secs: u64) -> CoreResult<Arc<Self>> {
            let client = redis::Client::open(redis_url)
                .map_err(|e| CoreError::Storage(format!("redis client: {e}")))?;
            let conn = redis::aio::ConnectionManager::new(client)
                .await
                .map_err(|e| CoreError::Storage(format!("redis connect: {e}")))?;
            Ok(Arc::new(Self {
                conn,
                ttl_secs,
                key_prefix: "dedup:".into(),
            }))
        }
    }

    #[async_trait]
    impl DedupStore for RedisDedup {
        async fn try_claim(&self, event_id: &str) -> CoreResult<bool> {
            let key = format!("{}{}", self.key_prefix, event_id);
            let mut conn = self.conn.clone();
            // SET key 1 EX ttl NX → Some(()) if set, None if exists
            let result: Option<String> = redis::cmd("SET")
                .arg(&key)
                .arg("1")
                .arg("EX")
                .arg(self.ttl_secs)
                .arg("NX")
                .query_async(&mut conn)
                .await
                .map_err(|e| CoreError::Storage(format!("redis SET NX: {e}")))?;
            Ok(result.is_some())
        }

        async fn release(&self, event_id: &str) -> CoreResult<()> {
            let key = format!("{}{}", self.key_prefix, event_id);
            let mut conn = self.conn.clone();
            let _: i64 = redis::cmd("DEL")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(|e| CoreError::Storage(format!("redis DEL: {e}")))?;
            Ok(())
        }

        async fn len_approx(&self) -> usize {
            0 // not cheap on Redis; metrics come from hit/miss counters
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn first_claim_succeeds_second_fails() {
        let d = InMemoryDedup::new(60);
        assert!(d.try_claim("evt-1").await.unwrap());
        assert!(!d.try_claim("evt-1").await.unwrap());
        assert!(d.try_claim("evt-2").await.unwrap());
    }
}
