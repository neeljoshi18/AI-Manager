//! Lightweight process-local metrics for observability baselines.
//!
//! In production these feed Prometheus via the ingestion / query services.
//! Spec §6 requires: throughput, latency percentiles, consumer lag, dedup ratios.

use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct IngestMetrics {
    pub accepted: AtomicU64,
    pub duplicates: AtomicU64,
    pub auth_failures: AtomicU64,
    pub rate_limited: AtomicU64,
    pub dead_lettered: AtomicU64,
    pub errors: AtomicU64,
    /// Unix seconds of last accepted ingest (0 = never). Used by product Connections UI.
    pub last_accepted_unix: AtomicU64,
    latency_samples_ms: Mutex<Vec<u64>>,
    dedup_hits: AtomicU64,
    dedup_misses: AtomicU64,
    per_tenant: DashMap<String, AtomicU64>,
}

impl IngestMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn record_latency(&self, ms: u64) {
        let mut samples = self.latency_samples_ms.lock();
        samples.push(ms);
        // Keep a rolling window to bound memory.
        if samples.len() > 50_000 {
            let drain = samples.len() - 25_000;
            samples.drain(0..drain);
        }
    }

    pub fn record_accepted(&self) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_accepted_unix.store(now, Ordering::Relaxed);
    }

    pub fn record_tenant(&self, tenant_id: &str) {
        self.per_tenant
            .entry(tenant_id.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dedup_hit(&self) {
        self.dedup_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dedup_miss(&self) {
        self.dedup_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut samples = self.latency_samples_ms.lock().clone();
        samples.sort_unstable();
        let p = |pct: f64| -> u64 {
            if samples.is_empty() {
                return 0;
            }
            let idx = ((samples.len() as f64 - 1.0) * pct).round() as usize;
            samples[idx.min(samples.len() - 1)]
        };
        MetricsSnapshot {
            accepted: self.accepted.load(Ordering::Relaxed),
            duplicates: self.duplicates.load(Ordering::Relaxed),
            auth_failures: self.auth_failures.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            dead_lettered: self.dead_lettered.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            last_accepted_unix: self.last_accepted_unix.load(Ordering::Relaxed),
            p50_ms: p(0.50),
            p95_ms: p(0.95),
            p99_ms: p(0.99),
            dedup_hits: self.dedup_hits.load(Ordering::Relaxed),
            dedup_misses: self.dedup_misses.load(Ordering::Relaxed),
            sample_count: samples.len() as u64,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub accepted: u64,
    pub duplicates: u64,
    pub auth_failures: u64,
    pub rate_limited: u64,
    pub dead_lettered: u64,
    pub errors: u64,
    /// Unix seconds of last accepted event; 0 if none yet.
    pub last_accepted_unix: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub dedup_hits: u64,
    pub dedup_misses: u64,
    pub sample_count: u64,
}

/// Simple timer helper.
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}
