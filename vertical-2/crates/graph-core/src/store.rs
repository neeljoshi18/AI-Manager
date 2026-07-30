use crate::acl::acl_allows;
use crate::error::{GraphError, GraphResult};
use crate::model::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn mark_event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool>;
    async fn apply_mutation(&self, mutation: GraphMutation) -> GraphResult<()>;

    async fn get_node(
        &self,
        ctx: &QueryContext,
        node_id: &str,
    ) -> GraphResult<Option<GraphNode>>;

    async fn get_state(
        &self,
        ctx: &QueryContext,
        node_id: &str,
        state_key: &str,
    ) -> GraphResult<Option<EntityState>>;

    async fn neighborhood(
        &self,
        ctx: &QueryContext,
        root_id: &str,
        hops: usize,
    ) -> GraphResult<Neighborhood>;

    async fn path(
        &self,
        ctx: &QueryContext,
        from_id: &str,
        to_id: &str,
        max_hops: usize,
    ) -> GraphResult<Option<GraphPath>>;

    async fn blockers(
        &self,
        ctx: &QueryContext,
        for_node_id: &str,
    ) -> GraphResult<Vec<GraphEdge>>;

    async fn count_nodes(&self, tenant_id: &str) -> GraphResult<u64>;
    async fn count_edges(&self, tenant_id: &str) -> GraphResult<u64>;
    async fn event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool>;

    /// ACL-filtered nodes of a given type (monitor / conflict / intent surfaces).
    async fn list_nodes_by_type(
        &self,
        ctx: &QueryContext,
        node_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphNode>>;

    /// ACL-filtered edges of a given type (e.g. BLOCKS).
    async fn list_edges_by_type(
        &self,
        ctx: &QueryContext,
        edge_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphEdge>>;

    /// ACL-filtered tenant snapshot for product Graph UI (not god-mode: still ACL-gated).
    async fn snapshot(
        &self,
        ctx: &QueryContext,
        node_limit: usize,
        edge_limit: usize,
    ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>)>;
}

/// Embedded graph durability snapshot (nodes/edges/states/applied event ids).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphPersistSnapshot {
    pub version: u32,
    pub saved_at: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub states: Vec<EntityState>,
    pub applied: Vec<(String, String)>, // (tenant_id, event_id)
}

pub struct InMemoryGraphStore {
    applied: DashMap<(String, String), ()>,
    nodes: DashMap<(String, String), GraphNode>,
    /// tenant → edges
    edges: DashMap<String, RwLock<Vec<GraphEdge>>>,
    /// (tenant, node, key) → state
    states: DashMap<(String, String, String), EntityState>,
    persist_path: RwLock<Option<std::path::PathBuf>>,
    dirty: AtomicU64,
}

impl InMemoryGraphStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            applied: DashMap::new(),
            nodes: DashMap::new(),
            edges: DashMap::new(),
            states: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
        })
    }

    pub fn set_persist_path(&self, path: Option<std::path::PathBuf>) {
        *self.persist_path.write() = path;
    }

    pub fn export_snapshot(&self) -> GraphPersistSnapshot {
        let nodes: Vec<GraphNode> = self.nodes.iter().map(|e| e.value().clone()).collect();
        let mut edges = Vec::new();
        for t in self.edges.iter() {
            edges.extend(t.value().read().iter().cloned());
        }
        let states: Vec<EntityState> = self.states.iter().map(|e| e.value().clone()).collect();
        let applied: Vec<(String, String)> = self
            .applied
            .iter()
            .map(|e| e.key().clone())
            .collect();
        GraphPersistSnapshot {
            version: 1,
            saved_at: Some(Utc::now().to_rfc3339()),
            nodes,
            edges,
            states,
            applied,
        }
    }

    pub fn import_snapshot(&self, snap: GraphPersistSnapshot) {
        for n in snap.nodes {
            self.nodes
                .insert((n.tenant_id.clone(), n.node_id.clone()), n);
        }
        for e in snap.edges {
            let tenant = e.tenant_id.clone();
            let list = self
                .edges
                .entry(tenant)
                .or_insert_with(|| RwLock::new(Vec::new()));
            list.write().push(e);
        }
        for s in snap.states {
            self.states.insert(
                (s.tenant_id.clone(), s.node_id.clone(), s.state_key.clone()),
                s,
            );
        }
        for (t, eid) in snap.applied {
            self.applied.insert((t, eid), ());
        }
    }

    pub fn save_to_path(&self, path: &Path) -> GraphResult<()> {
        let snap = self.export_snapshot();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GraphError::Storage(format!("graph persist mkdir {}: {e}", parent.display()))
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&snap)
            .map_err(|e| GraphError::Storage(format!("graph persist encode: {e}")))?;
        std::fs::write(&tmp, bytes)
            .map_err(|e| GraphError::Storage(format!("graph persist write: {e}")))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| GraphError::Storage(format!("graph persist rename: {e}")))?;
        self.dirty.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn load_from_path(&self, path: &Path) -> GraphResult<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let bytes = std::fs::read(path)
            .map_err(|e| GraphError::Storage(format!("graph persist read: {e}")))?;
        let snap: GraphPersistSnapshot = serde_json::from_slice(&bytes)
            .map_err(|e| GraphError::Storage(format!("graph persist decode: {e}")))?;
        let n = snap.nodes.len();
        let e = snap.edges.len();
        self.import_snapshot(snap);
        tracing::info!(
            path = %path.display(),
            nodes = n,
            edges = e,
            "loaded embedded V2 graph snapshot"
        );
        Ok(true)
    }

    fn maybe_persist(&self) {
        let n = self.dirty.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 3 != 0 {
            return;
        }
        let path = self.persist_path.read().clone();
        if let Some(p) = path {
            if let Err(e) = self.save_to_path(&p) {
                tracing::warn!(error = %e, "V2 graph persist failed");
            }
        }
    }

    fn visible_node(ctx: &QueryContext, n: &GraphNode) -> bool {
        n.tenant_id == ctx.tenant_id && acl_allows(ctx, n.is_private, &n.allowed_group_ids)
    }

    fn visible_edge(ctx: &QueryContext, e: &GraphEdge) -> bool {
        e.tenant_id == ctx.tenant_id && acl_allows(ctx, e.is_private, &e.allowed_group_ids)
    }

    fn active_edge(e: &GraphEdge, as_of: DateTime<Utc>) -> bool {
        e.valid_from <= as_of && e.valid_to.map(|t| t > as_of).unwrap_or(true)
    }
}

impl Default for InMemoryGraphStore {
    fn default() -> Self {
        Self {
            applied: DashMap::new(),
            nodes: DashMap::new(),
            edges: DashMap::new(),
            states: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl GraphStore for InMemoryGraphStore {
    async fn mark_event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool> {
        let key = (tenant_id.to_string(), event_id.to_string());
        if self.applied.contains_key(&key) {
            return Ok(false);
        }
        self.applied.insert(key, ());
        Ok(true)
    }

    async fn apply_mutation(&self, mutation: GraphMutation) -> GraphResult<()> {
        for n in mutation.nodes {
            self.nodes
                .insert((n.tenant_id.clone(), n.node_id.clone()), n);
        }
        for e in mutation.edges {
            let tenant = e.tenant_id.clone();
            let list = self
                .edges
                .entry(tenant)
                .or_insert_with(|| RwLock::new(Vec::new()));
            let mut v = list.write();
            if let Some(pos) = v.iter().position(|x| x.edge_id == e.edge_id) {
                v[pos] = e;
            } else {
                v.push(e);
            }
        }
        for s in mutation.states {
            let key = (
                s.tenant_id.clone(),
                s.node_id.clone(),
                s.state_key.clone(),
            );
            match self.states.get(&key) {
                Some(existing) if existing.as_of > s.as_of => {
                    // older event — ignore
                }
                Some(existing)
                    if existing.as_of == s.as_of
                        && existing.event_id != s.event_id
                        && existing.state_value != s.state_value =>
                {
                    // same timestamp: prefer higher sequence via event id lexicographic as tie-break
                    // Actually prefer CLOSED over OPEN if both at different times handled by as_of.
                    // Same as_of: keep existing unless new is "later" event id — use state priority.
                    let prio = |v: &str| match v {
                        "MERGED" => 3,
                        "CLOSED" => 2,
                        "OPEN" => 1,
                        _ => 0,
                    };
                    if prio(&s.state_value) >= prio(&existing.state_value) {
                        drop(existing);
                        self.states.insert(key, s);
                    }
                }
                _ => {
                    self.states.insert(key, s);
                }
            }
        }
        self.maybe_persist();
        Ok(())
    }

    async fn get_node(
        &self,
        ctx: &QueryContext,
        node_id: &str,
    ) -> GraphResult<Option<GraphNode>> {
        Ok(self
            .nodes
            .get(&(ctx.tenant_id.clone(), node_id.to_string()))
            .and_then(|n| {
                if Self::visible_node(ctx, &n) {
                    Some(n.clone())
                } else {
                    None
                }
            }))
    }

    async fn get_state(
        &self,
        ctx: &QueryContext,
        node_id: &str,
        state_key: &str,
    ) -> GraphResult<Option<EntityState>> {
        // Must be able to see the node (or state ACL).
        if self.get_node(ctx, node_id).await?.is_none() {
            // fail closed — also check if state exists but private
            if let Some(s) = self.states.get(&(
                ctx.tenant_id.clone(),
                node_id.to_string(),
                state_key.to_string(),
            )) {
                if !acl_allows(ctx, s.is_private, &s.allowed_group_ids) {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        }
        Ok(self
            .states
            .get(&(
                ctx.tenant_id.clone(),
                node_id.to_string(),
                state_key.to_string(),
            ))
            .and_then(|s| {
                if acl_allows(ctx, s.is_private, &s.allowed_group_ids) {
                    Some(s.clone())
                } else {
                    None
                }
            }))
    }

    async fn neighborhood(
        &self,
        ctx: &QueryContext,
        root_id: &str,
        hops: usize,
    ) -> GraphResult<Neighborhood> {
        let root = self
            .get_node(ctx, root_id)
            .await?
            .ok_or_else(|| GraphError::NotFound(format!("node {root_id}")))?;

        let hops = hops.clamp(1, 6);
        let now = Utc::now();
        let edges_guard = self.edges.get(&ctx.tenant_id);
        let all_edges: Vec<GraphEdge> = edges_guard
            .map(|g| g.read().clone())
            .unwrap_or_default();

        let mut seen_nodes: HashSet<String> = HashSet::new();
        let mut nodes_out: Vec<GraphNode> = vec![root.clone()];
        let mut edges_out: Vec<GraphEdge> = Vec::new();
        seen_nodes.insert(root_id.to_string());

        let mut frontier = vec![root_id.to_string()];
        for _ in 0..hops {
            let mut next = Vec::new();
            for nid in &frontier {
                for e in &all_edges {
                    if !Self::visible_edge(ctx, e) || !Self::active_edge(e, now) {
                        continue;
                    }
                    let other = if e.from_node_id == *nid {
                        Some(e.to_node_id.clone())
                    } else if e.to_node_id == *nid {
                        Some(e.from_node_id.clone())
                    } else {
                        None
                    };
                    let Some(other_id) = other else { continue };
                    // Both endpoints must be visible for edge to appear
                    let other_node = match self.get_node(ctx, &other_id).await? {
                        Some(n) => n,
                        None => continue,
                    };
                    if !edges_out.iter().any(|x| x.edge_id == e.edge_id) {
                        edges_out.push(e.clone());
                    }
                    if seen_nodes.insert(other_id.clone()) {
                        nodes_out.push(other_node);
                        next.push(other_id);
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
        let all_edges: Vec<GraphEdge> = self
            .edges
            .get(&ctx.tenant_id)
            .map(|g| g.read().clone())
            .unwrap_or_default();

        // BFS on undirected view of visible active edges
        let mut prev: HashMap<String, (String, GraphEdge)> = HashMap::new();
        let mut q = VecDeque::new();
        q.push_back(from_id.to_string());
        let mut dist: HashMap<String, usize> = HashMap::new();
        dist.insert(from_id.to_string(), 0);

        while let Some(cur) = q.pop_front() {
            let d = *dist.get(&cur).unwrap_or(&0);
            if d >= max_hops {
                continue;
            }
            for e in &all_edges {
                if !Self::visible_edge(ctx, e) || !Self::active_edge(e, now) {
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
                    // reconstruct
                    let mut nodes_ids = vec![to_id.to_string()];
                    let mut edges_path = Vec::new();
                    let mut walk = to_id.to_string();
                    while walk != from_id {
                        let (p, edge) = prev.get(&walk).unwrap().clone();
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
        let all_edges: Vec<GraphEdge> = self
            .edges
            .get(&ctx.tenant_id)
            .map(|g| g.read().clone())
            .unwrap_or_default();
        Ok(all_edges
            .into_iter()
            .filter(|e| {
                Self::visible_edge(ctx, e)
                    && Self::active_edge(e, now)
                    && (e.edge_type == "BLOCKS" || e.edge_type == "BLOCKED_BY")
                    && (e.from_node_id == for_node_id || e.to_node_id == for_node_id)
            })
            .collect())
    }

    async fn count_nodes(&self, tenant_id: &str) -> GraphResult<u64> {
        Ok(self
            .nodes
            .iter()
            .filter(|e| e.key().0 == tenant_id)
            .count() as u64)
    }

    async fn count_edges(&self, tenant_id: &str) -> GraphResult<u64> {
        Ok(self
            .edges
            .get(tenant_id)
            .map(|g| g.read().len() as u64)
            .unwrap_or(0))
    }

    async fn event_applied(&self, tenant_id: &str, event_id: &str) -> GraphResult<bool> {
        Ok(self
            .applied
            .contains_key(&(tenant_id.to_string(), event_id.to_string())))
    }

    async fn list_nodes_by_type(
        &self,
        ctx: &QueryContext,
        node_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphNode>> {
        let limit = limit.clamp(1, 500);
        let mut out: Vec<GraphNode> = self
            .nodes
            .iter()
            .filter(|e| {
                e.key().0 == ctx.tenant_id
                    && e.value().node_type.eq_ignore_ascii_case(node_type)
                    && Self::visible_node(ctx, e.value())
            })
            .map(|e| e.value().clone())
            .collect();
        out.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        out.truncate(limit);
        Ok(out)
    }

    async fn list_edges_by_type(
        &self,
        ctx: &QueryContext,
        edge_type: &str,
        limit: usize,
    ) -> GraphResult<Vec<GraphEdge>> {
        let limit = limit.clamp(1, 500);
        let now = Utc::now();
        let mut out: Vec<GraphEdge> = self
            .edges
            .get(&ctx.tenant_id)
            .map(|g| g.read().clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| {
                Self::visible_edge(ctx, e)
                    && Self::active_edge(e, now)
                    && e.edge_type.eq_ignore_ascii_case(edge_type)
            })
            .collect();
        out.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
        out.truncate(limit);
        Ok(out)
    }

    async fn snapshot(
        &self,
        ctx: &QueryContext,
        node_limit: usize,
        edge_limit: usize,
    ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>)> {
        let node_limit = node_limit.clamp(1, 2000);
        let edge_limit = edge_limit.clamp(1, 4000);
        let now = Utc::now();
        let mut nodes: Vec<GraphNode> = self
            .nodes
            .iter()
            .filter(|e| e.key().0 == ctx.tenant_id && Self::visible_node(ctx, e.value()))
            .map(|e| e.value().clone())
            .collect();
        nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        nodes.truncate(node_limit);

        let mut edges: Vec<GraphEdge> = self
            .edges
            .get(&ctx.tenant_id)
            .map(|g| g.read().clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| Self::visible_edge(ctx, e) && Self::active_edge(e, now))
            .collect();
        // Prefer edges whose endpoints are in the node set when truncated
        if nodes.len() == node_limit {
            let ids: HashSet<String> = nodes.iter().map(|n| n.node_id.clone()).collect();
            edges.retain(|e| ids.contains(&e.from_node_id) && ids.contains(&e.to_node_id));
        }
        edges.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
        edges.truncate(edge_limit);
        Ok((nodes, edges))
    }
}
