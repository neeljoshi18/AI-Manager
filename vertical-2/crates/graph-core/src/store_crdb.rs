//! CockroachDB-backed graph store (database context_graph).

use crate::acl::acl_allows;
use crate::error::{GraphError, GraphResult};
use crate::model::*;
use crate::store::GraphStore;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub struct CrdbGraphStore {
    pool: PgPool,
}

impl CrdbGraphStore {
    pub async fn connect(database_url: &str) -> GraphResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| GraphError::Storage(format!("crdb connect: {e}")))?;
        Ok(Arc::new(Self { pool }))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn load_node_raw(
        &self,
        tenant_id: &str,
        node_id: &str,
    ) -> GraphResult<Option<GraphNode>> {
        let row = sqlx::query(
            r#"
            SELECT tenant_id, node_id, node_type, display_name, resource_id,
                   properties_json, is_private, allowed_group_ids, acl_version
            FROM graph_node WHERE tenant_id = $1 AND node_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(format!("get node: {e}")))?;
        Ok(row.map(|r| row_to_node(&r)))
    }

    async fn load_edges(
        &self,
        tenant_id: &str,
    ) -> GraphResult<Vec<GraphEdge>> {
        let rows = sqlx::query(
            r#"
            SELECT tenant_id, edge_id, edge_type, from_node_id, to_node_id,
                   valid_from, valid_to, event_id, properties_json,
                   is_private, allowed_group_ids, acl_version
            FROM graph_edge WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(format!("list edges: {e}")))?;
        Ok(rows.iter().map(row_to_edge).collect())
    }
}

fn row_to_node(r: &sqlx::postgres::PgRow) -> GraphNode {
    let props: serde_json::Value = r.get("properties_json");
    let groups: Vec<String> = r.get("allowed_group_ids");
    GraphNode {
        tenant_id: r.get("tenant_id"),
        node_id: r.get("node_id"),
        node_type: r.get("node_type"),
        display_name: r.get("display_name"),
        resource_id: r.get("resource_id"),
        properties: props,
        is_private: r.get("is_private"),
        allowed_group_ids: groups,
        acl_version: r.get::<i64, _>("acl_version") as u64,
    }
}

fn row_to_edge(r: &sqlx::postgres::PgRow) -> GraphEdge {
    let props: serde_json::Value = r.get("properties_json");
    let groups: Vec<String> = r.get("allowed_group_ids");
    GraphEdge {
        tenant_id: r.get("tenant_id"),
        edge_id: r.get("edge_id"),
        edge_type: r.get("edge_type"),
        from_node_id: r.get("from_node_id"),
        to_node_id: r.get("to_node_id"),
        valid_from: r.get("valid_from"),
        valid_to: r.get("valid_to"),
        event_id: r.get("event_id"),
        properties: props,
        is_private: r.get("is_private"),
        allowed_group_ids: groups,
        acl_version: r.get::<i64, _>("acl_version") as u64,
    }
}

#[async_trait]
impl GraphStore for CrdbGraphStore {
    async fn mark_event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool> {
        let res = sqlx::query(
            r#"
            INSERT INTO projector_applied_events (tenant_id, event_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(format!("mark applied: {e}")))?;
        Ok(res.rows_affected() > 0)
    }

    async fn apply_mutation(&self, mutation: GraphMutation) -> GraphResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| GraphError::Storage(format!("tx: {e}")))?;

        for n in &mutation.nodes {
            sqlx::query(
                r#"
                INSERT INTO graph_node (
                    tenant_id, node_id, node_type, display_name, resource_id,
                    properties_json, is_private, allowed_group_ids, acl_version, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9, now())
                ON CONFLICT (tenant_id, node_id) DO UPDATE SET
                    node_type = EXCLUDED.node_type,
                    display_name = EXCLUDED.display_name,
                    resource_id = EXCLUDED.resource_id,
                    properties_json = EXCLUDED.properties_json,
                    is_private = EXCLUDED.is_private,
                    allowed_group_ids = EXCLUDED.allowed_group_ids,
                    acl_version = EXCLUDED.acl_version,
                    updated_at = now()
                "#,
            )
            .bind(&n.tenant_id)
            .bind(&n.node_id)
            .bind(&n.node_type)
            .bind(&n.display_name)
            .bind(&n.resource_id)
            .bind(&n.properties)
            .bind(n.is_private)
            .bind(&n.allowed_group_ids)
            .bind(n.acl_version as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| GraphError::Storage(format!("upsert node: {e}")))?;
        }

        for e in &mutation.edges {
            sqlx::query(
                r#"
                INSERT INTO graph_edge (
                    tenant_id, edge_id, edge_type, from_node_id, to_node_id,
                    valid_from, valid_to, event_id, properties_json,
                    is_private, allowed_group_ids, acl_version
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
                ON CONFLICT (tenant_id, edge_id) DO UPDATE SET
                    valid_from = EXCLUDED.valid_from,
                    valid_to = EXCLUDED.valid_to,
                    properties_json = EXCLUDED.properties_json,
                    is_private = EXCLUDED.is_private,
                    allowed_group_ids = EXCLUDED.allowed_group_ids,
                    acl_version = EXCLUDED.acl_version
                "#,
            )
            .bind(&e.tenant_id)
            .bind(&e.edge_id)
            .bind(&e.edge_type)
            .bind(&e.from_node_id)
            .bind(&e.to_node_id)
            .bind(e.valid_from)
            .bind(e.valid_to)
            .bind(&e.event_id)
            .bind(&e.properties)
            .bind(e.is_private)
            .bind(&e.allowed_group_ids)
            .bind(e.acl_version as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| GraphError::Storage(format!("upsert edge: {e}")))?;
        }

        for s in &mutation.states {
            // Temporal: only overwrite if as_of >= existing
            sqlx::query(
                r#"
                INSERT INTO entity_state (
                    tenant_id, node_id, state_key, state_value, as_of, event_id,
                    is_private, allowed_group_ids
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                ON CONFLICT (tenant_id, node_id, state_key) DO UPDATE SET
                    state_value = EXCLUDED.state_value,
                    as_of = EXCLUDED.as_of,
                    event_id = EXCLUDED.event_id,
                    is_private = EXCLUDED.is_private,
                    allowed_group_ids = EXCLUDED.allowed_group_ids
                WHERE entity_state.as_of < EXCLUDED.as_of
                   OR (entity_state.as_of = EXCLUDED.as_of AND entity_state.state_value IS DISTINCT FROM EXCLUDED.state_value
                       AND EXCLUDED.state_value IN ('CLOSED','MERGED'))
                "#,
            )
            .bind(&s.tenant_id)
            .bind(&s.node_id)
            .bind(&s.state_key)
            .bind(&s.state_value)
            .bind(s.as_of)
            .bind(&s.event_id)
            .bind(s.is_private)
            .bind(&s.allowed_group_ids)
            .execute(&mut *tx)
            .await
            .map_err(|e| GraphError::Storage(format!("upsert state: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| GraphError::Storage(format!("commit: {e}")))?;
        Ok(())
    }

    async fn get_node(
        &self,
        ctx: &QueryContext,
        node_id: &str,
    ) -> GraphResult<Option<GraphNode>> {
        let n = self.load_node_raw(&ctx.tenant_id, node_id).await?;
        Ok(n.filter(|n| acl_allows(ctx, n.is_private, &n.allowed_group_ids)))
    }

    async fn get_state(
        &self,
        ctx: &QueryContext,
        node_id: &str,
        state_key: &str,
    ) -> GraphResult<Option<EntityState>> {
        if self.get_node(ctx, node_id).await?.is_none() {
            return Ok(None);
        }
        let row = sqlx::query(
            r#"
            SELECT tenant_id, node_id, state_key, state_value, as_of, event_id,
                   is_private, allowed_group_ids
            FROM entity_state
            WHERE tenant_id = $1 AND node_id = $2 AND state_key = $3
            "#,
        )
        .bind(&ctx.tenant_id)
        .bind(node_id)
        .bind(state_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(format!("get state: {e}")))?;
        Ok(row.and_then(|r| {
            let groups: Vec<String> = r.get("allowed_group_ids");
            let is_private: bool = r.get("is_private");
            if !acl_allows(ctx, is_private, &groups) {
                return None;
            }
            Some(EntityState {
                tenant_id: r.get("tenant_id"),
                node_id: r.get("node_id"),
                state_key: r.get("state_key"),
                state_value: r.get("state_value"),
                as_of: r.get("as_of"),
                event_id: r.get("event_id"),
                is_private,
                allowed_group_ids: groups,
            })
        }))
    }

    async fn neighborhood(
        &self,
        ctx: &QueryContext,
        root_id: &str,
        hops: usize,
    ) -> GraphResult<Neighborhood> {
        // Reuse in-memory algorithm over loaded visible graph subset for correctness.
        let root = self
            .get_node(ctx, root_id)
            .await?
            .ok_or_else(|| GraphError::NotFound(format!("node {root_id}")))?;
        let hops = hops.clamp(1, 6);
        let now = Utc::now();
        let all_edges = self.load_edges(&ctx.tenant_id).await?;

        let mut seen: HashSet<String> = HashSet::new();
        let mut nodes_out = vec![root.clone()];
        let mut edges_out = Vec::new();
        seen.insert(root_id.to_string());
        let mut frontier = vec![root_id.to_string()];

        for _ in 0..hops {
            let mut next = Vec::new();
            for nid in &frontier {
                for e in &all_edges {
                    if !acl_allows(ctx, e.is_private, &e.allowed_group_ids) {
                        continue;
                    }
                    if e.valid_from > now || e.valid_to.map(|t| t <= now).unwrap_or(false) {
                        continue;
                    }
                    let other = if e.from_node_id == *nid {
                        e.to_node_id.clone()
                    } else if e.to_node_id == *nid {
                        e.from_node_id.clone()
                    } else {
                        continue;
                    };
                    let Some(on) = self.get_node(ctx, &other).await? else {
                        continue;
                    };
                    if !edges_out.iter().any(|x: &GraphEdge| x.edge_id == e.edge_id) {
                        edges_out.push(e.clone());
                    }
                    if seen.insert(other.clone()) {
                        nodes_out.push(on);
                        next.push(other);
                    }
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        Ok(Neighborhood {
            root,
            nodes: nodes_out,
            edges: edges_out,
            hops,
        })
    }

    async fn path(
        &self,
        ctx: &QueryContext,
        from_id: &str,
        to_id: &str,
        max_hops: usize,
    ) -> GraphResult<Option<GraphPath>> {
        let max_hops = max_hops.clamp(1, 6);
        if self.get_node(ctx, from_id).await?.is_none()
            || self.get_node(ctx, to_id).await?.is_none()
        {
            return Ok(None);
        }
        let now = Utc::now();
        let all_edges = self.load_edges(&ctx.tenant_id).await?;
        let mut prev: HashMap<String, (String, GraphEdge)> = HashMap::new();
        let mut q = VecDeque::new();
        let mut dist: HashMap<String, usize> = HashMap::new();
        q.push_back(from_id.to_string());
        dist.insert(from_id.to_string(), 0);
        while let Some(cur) = q.pop_front() {
            let d = dist[&cur];
            if d >= max_hops {
                continue;
            }
            for e in &all_edges {
                if !acl_allows(ctx, e.is_private, &e.allowed_group_ids) {
                    continue;
                }
                if e.valid_from > now || e.valid_to.map(|t| t <= now).unwrap_or(false) {
                    continue;
                }
                let other = if e.from_node_id == cur {
                    e.to_node_id.clone()
                } else if e.to_node_id == cur {
                    e.from_node_id.clone()
                } else {
                    continue;
                };
                if self.get_node(ctx, &other).await?.is_none() {
                    continue;
                }
                if dist.contains_key(&other) {
                    continue;
                }
                dist.insert(other.clone(), d + 1);
                prev.insert(other.clone(), (cur.clone(), e.clone()));
                if other == to_id {
                    let mut nodes_ids = vec![to_id.to_string()];
                    let mut edges_path = Vec::new();
                    let mut walk = to_id.to_string();
                    while walk != from_id {
                        let (p, edge) = prev[&walk].clone();
                        edges_path.push(edge);
                        nodes_ids.push(p.clone());
                        walk = p;
                    }
                    nodes_ids.reverse();
                    edges_path.reverse();
                    let mut nodes = Vec::new();
                    for id in nodes_ids {
                        if let Some(n) = self.get_node(ctx, &id).await? {
                            nodes.push(n);
                        }
                    }
                    return Ok(Some(GraphPath {
                        nodes,
                        edges: edges_path,
                    }));
                }
                q.push_back(other);
            }
        }
        Ok(None)
    }

    async fn blockers(
        &self,
        ctx: &QueryContext,
        for_node_id: &str,
    ) -> GraphResult<Vec<GraphEdge>> {
        let now = Utc::now();
        let edges = self.load_edges(&ctx.tenant_id).await?;
        Ok(edges
            .into_iter()
            .filter(|e| {
                acl_allows(ctx, e.is_private, &e.allowed_group_ids)
                    && e.valid_from <= now
                    && e.valid_to.map(|t| t > now).unwrap_or(true)
                    && (e.edge_type == "BLOCKS" || e.edge_type == "BLOCKED_BY")
                    && (e.from_node_id == for_node_id || e.to_node_id == for_node_id)
            })
            .collect())
    }

    async fn count_nodes(&self, tenant_id: &str) -> GraphResult<u64> {
        let n: (i64,) = sqlx::query_as("SELECT count(*) FROM graph_node WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(n.0 as u64)
    }

    async fn count_edges(&self, tenant_id: &str) -> GraphResult<u64> {
        let n: (i64,) = sqlx::query_as("SELECT count(*) FROM graph_edge WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(n.0 as u64)
    }

    async fn event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool> {
        let n: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM projector_applied_events WHERE tenant_id=$1 AND event_id=$2",
        )
        .bind(tenant_id)
        .bind(event_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(n.0 > 0)
    }

    async fn list_nodes_by_type(
        &self,
        ctx: &QueryContext,
        node_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphNode>> {
        let limit = limit.clamp(1, 500) as i64;
        let rows = sqlx::query(
            r#"
            SELECT tenant_id, node_id, node_type, display_name, resource_id,
                   properties_json, is_private, allowed_group_ids, acl_version
            FROM graph_node
            WHERE tenant_id = $1 AND lower(node_type) = lower($2)
            ORDER BY node_id
            LIMIT $3
            "#,
        )
        .bind(&ctx.tenant_id)
        .bind(node_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(format!("list nodes by type: {e}")))?;
        Ok(rows
            .iter()
            .map(row_to_node)
            .filter(|n| acl_allows(ctx, n.is_private, &n.allowed_group_ids))
            .collect())
    }

    async fn list_edges_by_type(
        &self,
        ctx: &QueryContext,
        edge_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphEdge>> {
        let limit = limit.clamp(1, 500);
        let now = Utc::now();
        let edges = self.load_edges(&ctx.tenant_id).await?;
        let mut out: Vec<GraphEdge> = edges
            .into_iter()
            .filter(|e| {
                acl_allows(ctx, e.is_private, &e.allowed_group_ids)
                    && e.valid_from <= now
                    && e.valid_to.map(|t| t > now).unwrap_or(true)
                    && e.edge_type.eq_ignore_ascii_case(edge_type)
            })
            .collect();
        out.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
        out.truncate(limit);
        Ok(out)
    }
}

pub struct CrdbMembership {
    pool: PgPool,
}

impl CrdbMembership {
    pub async fn connect(database_url: &str) -> GraphResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| GraphError::Storage(format!("membership connect: {e}")))?;
        Ok(Arc::new(Self { pool }))
    }
}

#[async_trait]
impl crate::membership::MembershipStore for CrdbMembership {
    async fn set_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        groups: &[String],
    ) -> GraphResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        sqlx::query("DELETE FROM user_membership WHERE tenant_id=$1 AND global_user_id=$2")
            .bind(tenant_id)
            .bind(global_user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        for g in groups {
            sqlx::query(
                "INSERT INTO user_membership (tenant_id, global_user_id, group_id) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
            )
            .bind(tenant_id)
            .bind(global_user_id)
            .bind(g)
            .execute(&mut *tx)
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn add_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        sqlx::query(
            "INSERT INTO user_membership (tenant_id, global_user_id, group_id) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn remove_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        sqlx::query(
            "DELETE FROM user_membership WHERE tenant_id=$1 AND global_user_id=$2 AND group_id=$3",
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> GraphResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT group_id FROM user_membership WHERE tenant_id=$1 AND global_user_id=$2",
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GraphError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
