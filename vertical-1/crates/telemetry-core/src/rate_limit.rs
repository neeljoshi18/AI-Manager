//! Token-bucket / sliding-window rate limiter (Spec §3.1).
//!
//! Tiered limit: 10,000 requests/minute per tenant ID.
//! Exceeded limits → 429 + Retry-After.

use crate::error::{CoreError, CoreResult};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Returns Ok(()) if under limit, or RateLimited error with retry-after.
    async fn check(&self, tenant_id: &str) -> CoreResult<()>;
}

/// Sliding-window limiter in process memory.
pub struct InMemoryRateLimiter {
    limit_per_window: u64,
    window: Duration,
    /// tenant → timestamps of recent requests
    windows: DashMap<String, Mutex<VecDeque<Instant>>>,
}

impl InMemoryRateLimiter {
    pub fn new(limit_per_minute: u64) -> Arc<Self> {
        Arc::new(Self {
            limit_per_window: limit_per_minute,
            window: Duration::from_secs(60),
            windows: DashMap::new(),
        })
    }

    /// Test helper: very small window for unit tests.
    pub fn with_window(limit: u64, window: Duration) -> Arc<Self> {
        Arc::new(Self {
            limit_per_window: limit,
            window,
            windows: DashMap::new(),
        })
    }
}

#[async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn check(&self, tenant_id: &str) -> CoreResult<()> {
        let now = Instant::now();
        let entry = self
            .windows
            .entry(tenant_id.to_string())
            .or_insert_with(|| Mutex::new(VecDeque::new()));
        let mut q = entry.lock();

        while let Some(front) = q.front() {
            if now.duration_since(*front) > self.window {
                q.pop_front();
            } else {
                break;
            }
        }

        if q.len() as u64 >= self.limit_per_window {
            let retry = q
                .front()
                .map(|t| {
                    self.window
                        .saturating_sub(now.duration_since(*t))
                        .as_secs()
                        .max(1)
                })
                .unwrap_or(1);
            return Err(CoreError::RateLimited {
                retry_after_secs: retry,
            });
        }

        q.push_back(now);
        Ok(())
    }
}

#[cfg(feature = "production")]
pub mod redis_impl {
    use super::*;

    /// Fixed-window counter in Redis: INCR + EXPIRE on first hit.
    pub struct RedisRateLimiter {
        conn: redis::aio::ConnectionManager,
        limit_per_minute: u64,
    }

    impl RedisRateLimiter {
        pub async fn connect(redis_url: &str, limit_per_minute: u64) -> CoreResult<Arc<Self>> {
            let client = redis::Client::open(redis_url)
                .map_err(|e| CoreError::Storage(format!("redis client: {e}")))?;
            let conn = redis::aio::ConnectionManager::new(client)
                .await
                .map_err(|e| CoreError::Storage(format!("redis connect: {e}")))?;
            Ok(Arc::new(Self {
                conn,
                limit_per_minute,
            }))
        }
    }

    #[async_trait]
    impl RateLimiter for RedisRateLimiter {
        async fn check(&self, tenant_id: &str) -> CoreResult<()> {
            let key = format!(
                "ratelimit:{}:{}",
                tenant_id,
                chrono::Utc::now().timestamp() / 60
            );
            let mut conn = self.conn.clone();
            let count: u64 = redis::cmd("INCR")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(|e| CoreError::Storage(format!("redis INCR: {e}")))?;
            if count == 1 {
                let _: () = redis::cmd("EXPIRE")
                    .arg(&key)
                    .arg(60)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| CoreError::Storage(format!("redis EXPIRE: {e}")))?;
            }
            if count > self.limit_per_minute {
                return Err(CoreError::RateLimited {
                    retry_after_secs: 60 - (chrono::Utc::now().timestamp() % 60) as u64,
                });
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enforces_limit() {
        let rl = InMemoryRateLimiter::with_window(3, Duration::from_secs(60));
        rl.check("t1").await.unwrap();
        rl.check("t1").await.unwrap();
        rl.check("t1").await.unwrap();
        assert!(matches!(
            rl.check("t1").await,
            Err(CoreError::RateLimited { .. })
        ));
        // Other tenants unaffected.
        rl.check("t2").await.unwrap();
    }
}
