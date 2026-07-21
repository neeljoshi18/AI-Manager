//! Object store for raw webhook payloads and DLQ (Spec §3.2 / Challenge 3).
//!
//! Raw unstructured JSON is isolated to cold object storage; only the S3 URI
//! is retained on the canonical event.

use crate::error::{CoreError, CoreResult};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Persist raw bytes; returns a URI (`s3://bucket/key` or `mem://key`).
    async fn put(&self, key: &str, bytes: &[u8]) -> CoreResult<String>;

    async fn get(&self, uri: &str) -> CoreResult<Vec<u8>>;

    /// Convenience for webhook raw vault path layout.
    async fn put_raw_payload(
        &self,
        tenant_id: &str,
        provider: &str,
        event_id: &str,
        bytes: &[u8],
    ) -> CoreResult<String> {
        let day = Utc::now().format("%Y/%m/%d");
        let key = format!("raw/{tenant_id}/{provider}/{day}/{event_id}.json");
        self.put(&key, bytes).await
    }

    async fn put_dlq(
        &self,
        tenant_id: &str,
        reason: &str,
        bytes: &[u8],
    ) -> CoreResult<String> {
        let day = Utc::now().format("%Y/%m/%d");
        let id = Uuid::new_v4();
        let key = format!("dlq/{tenant_id}/{day}/{reason}/{id}.json");
        self.put(&key, bytes).await
    }
}

pub struct InMemoryObjectStore {
    bucket: String,
    objects: DashMap<String, Vec<u8>>,
}

impl InMemoryObjectStore {
    pub fn new(bucket: &str) -> Arc<Self> {
        Arc::new(Self {
            bucket: bucket.to_string(),
            objects: DashMap::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }
}

#[async_trait]
impl ObjectStore for InMemoryObjectStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> CoreResult<String> {
        let uri = format!("s3://{}/{}", self.bucket, key);
        self.objects.insert(uri.clone(), bytes.to_vec());
        Ok(uri)
    }

    async fn get(&self, uri: &str) -> CoreResult<Vec<u8>> {
        self.objects
            .get(uri)
            .map(|v| v.clone())
            .ok_or_else(|| CoreError::NotFound(format!("object {uri}")))
    }
}

/// Local filesystem object store (dev fallback).
pub struct LocalFsObjectStore {
    root: std::path::PathBuf,
    bucket: String,
}

impl LocalFsObjectStore {
    pub fn new(root: impl Into<std::path::PathBuf>, bucket: &str) -> Arc<Self> {
        let root = root.into();
        let _ = std::fs::create_dir_all(root.join(bucket));
        Arc::new(Self {
            root,
            bucket: bucket.to_string(),
        })
    }
}

#[async_trait]
impl ObjectStore for LocalFsObjectStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> CoreResult<String> {
        let path = self.root.join(&self.bucket).join(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CoreError::ObjectStore(e.to_string()))?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|e| CoreError::ObjectStore(e.to_string()))?;
        Ok(format!("s3://{}/{}", self.bucket, key))
    }

    async fn get(&self, uri: &str) -> CoreResult<Vec<u8>> {
        let prefix = format!("s3://{}/", self.bucket);
        let key = uri
            .strip_prefix(&prefix)
            .ok_or_else(|| CoreError::ObjectStore(format!("bad uri {uri}")))?;
        let path = self.root.join(&self.bucket).join(key);
        tokio::fs::read(path)
            .await
            .map_err(|e| CoreError::ObjectStore(e.to_string()))
    }
}
