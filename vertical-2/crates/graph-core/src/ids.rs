use sha2::{Digest, Sha256};

pub fn person_node_id(global_user_id: &str) -> String {
    if global_user_id.is_empty() {
        "person:unknown".into()
    } else {
        format!("person:{global_user_id}")
    }
}

pub fn person_from_provider(provider_user_id: &str) -> String {
    format!("person:prov:{provider_user_id}")
}

pub fn repo_node_id(resource: &str) -> String {
    format!("repo:{resource}")
}

pub fn pr_node_id(resource_id: &str) -> String {
    format!("pr:{resource_id}")
}

pub fn issue_node_id(resource_id: &str) -> String {
    format!("issue:{resource_id}")
}

pub fn team_node_id(team: &str) -> String {
    format!("team:{team}")
}

pub fn channel_node_id(channel: &str) -> String {
    format!("channel:{channel}")
}

pub fn commit_node_id(repo: &str, sha: &str) -> String {
    format!("commit:{repo}:{sha}")
}

/// Deterministic edge id for idempotent upserts.
pub fn edge_id(
    tenant_id: &str,
    edge_type: &str,
    from: &str,
    to: &str,
    event_id: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(tenant_id.as_bytes());
    h.update(b"|");
    h.update(edge_type.as_bytes());
    h.update(b"|");
    h.update(from.as_bytes());
    h.update(b"|");
    h.update(to.as_bytes());
    h.update(b"|");
    h.update(event_id.as_bytes());
    format!("e_{}", hex::encode(h.finalize()))
}

/// Stable edge id for long-lived relations (type+endpoints only).
pub fn stable_edge_id(tenant_id: &str, edge_type: &str, from: &str, to: &str) -> String {
    let mut h = Sha256::new();
    h.update(tenant_id.as_bytes());
    h.update(b"|");
    h.update(edge_type.as_bytes());
    h.update(b"|");
    h.update(from.as_bytes());
    h.update(b"|");
    h.update(to.as_bytes());
    format!("es_{}", hex::encode(h.finalize()))
}
