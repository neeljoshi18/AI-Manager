use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub runtime_mode: String,
    pub http_bind: String,
    pub cockroach_url: Option<String>,
    /// Vertical 1 identity DB (defaultdb) for live group reads.
    pub v1_cockroach_url: Option<String>,
    pub kafka_brokers: Option<String>,
    pub consumer_group: String,
    pub max_hops: usize,
    pub default_hops: usize,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            runtime_mode: "embedded".into(),
            http_bind: "0.0.0.0:18082".into(),
            cockroach_url: Some(
                "postgresql://root@127.0.0.1:26257/context_graph?sslmode=disable".into(),
            ),
            v1_cockroach_url: Some(
                "postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable".into(),
            ),
            kafka_brokers: Some("127.0.0.1:19092".into()),
            consumer_group: "v2-graph-projector".into(),
            max_hops: 6,
            default_hops: 3,
        }
    }
}

impl GraphConfig {
    pub fn from_env() -> Self {
        let mut c = Self::default();
        if let Ok(v) = std::env::var("RUNTIME_MODE") {
            c.runtime_mode = v;
        }
        if let Ok(v) = std::env::var("GRAPH_HTTP_BIND").or_else(|_| std::env::var("HTTP_BIND")) {
            c.http_bind = v;
        }
        if let Ok(v) = std::env::var("COCKROACH_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
        {
            // Prefer context_graph DB if URL points at defaultdb
            c.cockroach_url = Some(v.replace("/defaultdb", "/context_graph"));
        }
        if let Ok(v) = std::env::var("KAFKA_BROKERS") {
            c.kafka_brokers = Some(v);
        }
        if let Ok(v) = std::env::var("CONSUMER_GROUP") {
            c.consumer_group = v;
        }
        c
    }

    pub fn is_embedded(&self) -> bool {
        self.runtime_mode.eq_ignore_ascii_case("embedded")
    }
}
