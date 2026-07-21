use serde::{Deserialize, Serialize};

/// Global configuration for Vertical 1 services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub runtime_mode: String,
    pub http_bind: String,
    pub query_bind: String,
    pub consumer_group: String,

    /// Rate limit: requests per minute per tenant (spec: 10_000).
    pub rate_limit_per_minute: u64,
    /// Dedup TTL in seconds (spec: 86_400 = 24h).
    pub dedup_ttl_secs: u64,

    pub redis_url: Option<String>,
    pub kafka_brokers: Option<String>,
    pub cockroach_url: Option<String>,
    pub clickhouse_url: Option<String>,
    pub clickhouse_database: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,

    /// Skip HMAC verification (ONLY for local synthetic tests).
    pub skip_auth: bool,

    /// When true in production mode, also write the analytical store inline
    /// (useful for single-process demos). Prefer a dedicated consumer in prod.
    pub inline_consumer: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            runtime_mode: "embedded".into(),
            http_bind: "0.0.0.0:18080".into(),
            query_bind: "0.0.0.0:18081".into(),
            consumer_group: "v1-clickhouse-writer".into(),
            rate_limit_per_minute: 10_000,
            dedup_ttl_secs: 86_400,
            redis_url: Some("redis://127.0.0.1:6379".into()),
            kafka_brokers: Some("127.0.0.1:19092".into()),
            cockroach_url: Some(
                "postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable".into(),
            ),
            clickhouse_url: Some("http://127.0.0.1:8123".into()),
            clickhouse_database: "enterprise_telemetry".into(),
            clickhouse_user: "default".into(),
            clickhouse_password: "vertical1".into(),
            s3_endpoint: Some("http://127.0.0.1:9002".into()),
            s3_bucket: "ai-manager-telemetry".into(),
            s3_region: "us-east-1".into(),
            s3_access_key: Some("minioadmin".into()),
            s3_secret_key: Some("minioadmin".into()),
            skip_auth: false,
            inline_consumer: true,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("RUNTIME_MODE") {
            cfg.runtime_mode = v;
        }
        if let Ok(v) = std::env::var("HTTP_BIND") {
            cfg.http_bind = v;
        }
        if let Ok(v) = std::env::var("QUERY_BIND") {
            cfg.query_bind = v;
        }
        if let Ok(v) = std::env::var("CONSUMER_GROUP") {
            cfg.consumer_group = v;
        }
        if let Ok(v) = std::env::var("RATE_LIMIT_PER_MINUTE") {
            if let Ok(n) = v.parse() {
                cfg.rate_limit_per_minute = n;
            }
        }
        if let Ok(v) = std::env::var("DEDUP_TTL_SECS") {
            if let Ok(n) = v.parse() {
                cfg.dedup_ttl_secs = n;
            }
        }
        if let Ok(v) = std::env::var("REDIS_URL") {
            cfg.redis_url = Some(v);
        }
        if let Ok(v) = std::env::var("KAFKA_BROKERS") {
            cfg.kafka_brokers = Some(v);
        }
        if let Ok(v) = std::env::var("COCKROACH_URL").or_else(|_| std::env::var("DATABASE_URL")) {
            cfg.cockroach_url = Some(v);
        }
        if let Ok(v) = std::env::var("CLICKHOUSE_URL") {
            cfg.clickhouse_url = Some(v);
        }
        if let Ok(v) = std::env::var("CLICKHOUSE_DATABASE") {
            cfg.clickhouse_database = v;
        }
        if let Ok(v) = std::env::var("CLICKHOUSE_USER") {
            cfg.clickhouse_user = v;
        }
        if let Ok(v) = std::env::var("CLICKHOUSE_PASSWORD") {
            cfg.clickhouse_password = v;
        }
        if let Ok(v) = std::env::var("S3_ENDPOINT") {
            cfg.s3_endpoint = Some(v);
        }
        if let Ok(v) = std::env::var("S3_BUCKET") {
            cfg.s3_bucket = v;
        }
        if let Ok(v) = std::env::var("S3_REGION") {
            cfg.s3_region = v;
        }
        if let Ok(v) = std::env::var("S3_ACCESS_KEY") {
            cfg.s3_access_key = Some(v);
        }
        if let Ok(v) = std::env::var("S3_SECRET_KEY") {
            cfg.s3_secret_key = Some(v);
        }
        cfg.skip_auth = std::env::var("SKIP_AUTH")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        cfg.inline_consumer = std::env::var("INLINE_CONSUMER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(cfg.inline_consumer);
        // Explicit false
        if std::env::var("INLINE_CONSUMER")
            .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
            .unwrap_or(false)
        {
            cfg.inline_consumer = false;
        }
        cfg
    }

    pub fn is_embedded(&self) -> bool {
        self.runtime_mode.eq_ignore_ascii_case("embedded")
    }

    pub fn is_production(&self) -> bool {
        self.runtime_mode.eq_ignore_ascii_case("production")
    }
}
