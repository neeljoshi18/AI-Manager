//! MinIO / S3 object store for raw webhook vault + DLQ.

use crate::error::{CoreError, CoreResult};
use crate::object_store::ObjectStore;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::sync::Arc;
use tracing::info;

pub struct S3ObjectStore {
    client: Client,
    bucket: String,
}

impl S3ObjectStore {
    pub async fn connect(
        endpoint: &str,
        region: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
    ) -> CoreResult<Arc<Self>> {
        let creds = Credentials::new(access_key, secret_key, None, None, "vertical1");
        let conf = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region.to_string()))
            .endpoint_url(endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();
        let client = Client::from_conf(conf);

        // Ensure bucket exists (MinIO).
        match client.head_bucket().bucket(bucket).send().await {
            Ok(_) => {}
            Err(_) => {
                let _ = client.create_bucket().bucket(bucket).send().await;
                info!(bucket, "created s3/minio bucket");
            }
        }

        Ok(Arc::new(Self {
            client,
            bucket: bucket.to_string(),
        }))
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> CoreResult<String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes.to_vec()))
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| CoreError::ObjectStore(format!("s3 put: {e}")))?;
        Ok(format!("s3://{}/{}", self.bucket, key))
    }

    async fn get(&self, uri: &str) -> CoreResult<Vec<u8>> {
        let prefix = format!("s3://{}/", self.bucket);
        let key = uri
            .strip_prefix(&prefix)
            .ok_or_else(|| CoreError::ObjectStore(format!("bad uri {uri}")))?;
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| CoreError::ObjectStore(format!("s3 get: {e}")))?;
        let data = out
            .body
            .collect()
            .await
            .map_err(|e| CoreError::ObjectStore(format!("s3 body: {e}")))?
            .into_bytes()
            .to_vec();
        Ok(data)
    }
}
