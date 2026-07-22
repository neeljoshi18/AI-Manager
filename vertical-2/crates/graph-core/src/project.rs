//! Map V1 canonical events → graph mutations and apply via GraphStore.

use crate::error::GraphResult;
use crate::ids::*;
use crate::membership::{apply_membership_change, MembershipStore};
use crate::model::*;
use crate::store::GraphStore;
use crate::v1_event::{V1AclRevocation, V1CanonicalEvent};
use serde_json::json;
use std::sync::Arc;
use tracing::debug;

pub struct ProjectEngine {
    pub store: Arc<dyn GraphStore>,
    pub membership: Arc<dyn MembershipStore>,
}

impl ProjectEngine {
    pub fn new(store: Arc<dyn GraphStore>, membership: Arc<dyn MembershipStore>) -> Self {
        Self { store, membership }
    }

    pub async fn project_event(&self, event: &V1CanonicalEvent) -> GraphResult<ProjectOutcome> {
        let first = self
            .store
            .mark_event_applied(&event.tenant_id, &event.event_id)
            .await?;
        if !first {
            return Ok(ProjectOutcome {
                event_id: event.event_id.clone(),
                tenant_id: event.tenant_id.clone(),
                status: ProjectStatus::Duplicate,
                nodes_upserted: 0,
                edges_upserted: 0,
            });
        }

        // Identity / ACL membership side channel
        if event.category.eq_ignore_ascii_case("identity")
            || event.event_type.contains("identity")
            || event.event_type.contains("member")
        {
            self.try_membership_from_identity_event(event).await?;
        }

        let mutation = map_event(event);
        if mutation.nodes.is_empty() && mutation.edges.is_empty() && mutation.states.is_empty() {
            debug!(event_type = %event.event_type, "no graph mapping; skipped");
            return Ok(ProjectOutcome {
                event_id: event.event_id.clone(),
                tenant_id: event.tenant_id.clone(),
                status: ProjectStatus::Skipped,
                nodes_upserted: 0,
                edges_upserted: 0,
            });
        }

        let n = mutation.nodes.len();
        let e = mutation.edges.len();
        self.store.apply_mutation(mutation).await?;
        Ok(ProjectOutcome {
            event_id: event.event_id.clone(),
            tenant_id: event.tenant_id.clone(),
            status: ProjectStatus::Applied,
            nodes_upserted: n,
            edges_upserted: e,
        })
    }

    pub async fn project_acl_revocation(
        &self,
        rev: &V1AclRevocation,
    ) -> GraphResult<ProjectOutcome> {
        let first = self
            .store
            .mark_event_applied(&rev.tenant_id, &rev.event_id)
            .await?;
        if !first {
            return Ok(ProjectOutcome {
                event_id: rev.event_id.clone(),
                tenant_id: rev.tenant_id.clone(),
                status: ProjectStatus::Duplicate,
                nodes_upserted: 0,
                edges_upserted: 0,
            });
        }
        apply_membership_change(
            self.membership.as_ref(),
            &rev.tenant_id,
            &rev.global_user_id,
            &rev.group_id,
            &rev.change_type,
        )
        .await?;
        Ok(ProjectOutcome {
            event_id: rev.event_id.clone(),
            tenant_id: rev.tenant_id.clone(),
            status: ProjectStatus::Applied,
            nodes_upserted: 0,
            edges_upserted: 0,
        })
    }

    async fn try_membership_from_identity_event(
        &self,
        event: &V1CanonicalEvent,
    ) -> GraphResult<()> {
        let team = event
            .attributes
            .get("team")
            .or_else(|| event.attributes.get("group_name"))
            .or_else(|| event.attributes.get("channel"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let member = event
            .attributes
            .get("member")
            .or_else(|| event.attributes.get("user"))
            .and_then(|v| v.as_str())
            .unwrap_or(event.actor.global_user_id.as_str());
        if team.is_empty() || member.is_empty() {
            return Ok(());
        }
        let et = event.event_type.to_ascii_lowercase();
        let change = if et.contains("remove") || et.contains("left") || et.contains("deleted") {
            "removed_from_group"
        } else if et.contains("add") || et.contains("join") || et.contains("created") {
            "added_to_group"
        } else {
            return Ok(());
        };
        // Membership uses global user id when available
        let uid = if !event.actor.global_user_id.is_empty() {
            event.actor.global_user_id.as_str()
        } else {
            member
        };
        let _ = apply_membership_change(
            self.membership.as_ref(),
            &event.tenant_id,
            uid,
            team,
            change,
        )
        .await;
        Ok(())
    }
}

pub fn map_event(event: &V1CanonicalEvent) -> GraphMutation {
    let et = event.event_type.as_str();
    let et_l = et.to_ascii_lowercase();
    if et.starts_with("pull_request.") || et.contains("merge_request.") {
        return map_pull_request(event);
    }
    // Identity / team membership → MEMBER_OF Person→Team (before generic issue routes)
    if event.category.eq_ignore_ascii_case("identity")
        || et_l.contains("identity")
        || (et_l.contains("member") && !et_l.contains("comment"))
    {
        let m = map_team_membership(event);
        if !m.nodes.is_empty() || !m.edges.is_empty() {
            return m;
        }
    }
    if et.starts_with("issue.")
        || et_l.contains("jira:")
        || et.starts_with("linear.")
        || et_l.contains("issue.assigned")
        || et_l.contains("issue_assigned")
    {
        return map_issue(event);
    }
    if et == "push" || et.starts_with("push") {
        return map_push(event);
    }
    if et.starts_with("slack.") || et.starts_with("teams.") {
        return map_communication(event);
    }
    if et_l.contains("block") {
        return map_blocker_hint(event);
    }
    GraphMutation::default()
}

fn acl_fields(event: &V1CanonicalEvent) -> (bool, Vec<String>, u64) {
    (
        event.acl.is_private,
        event.acl.allowed_group_ids.clone(),
        event.acl.acl_version,
    )
}

fn person_node(event: &V1CanonicalEvent) -> GraphNode {
    let (_is_private, groups, ver) = acl_fields(event);
    let key = event.person_key();
    let nid = if key.starts_with("prov:") {
        person_from_provider(key.trim_start_matches("prov:"))
    } else {
        person_node_id(&key)
    };
    GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: nid,
        node_type: "Person".into(),
        display_name: if event.actor.display_name.is_empty() {
            event.actor.provider_user_id.clone()
        } else {
            event.actor.display_name.clone()
        },
        resource_id: event.actor.provider_user_id.clone(),
        properties: json!({
            "email": event.actor.email,
            "provider_user_id": event.actor.provider_user_id,
        }),
        is_private: false, // people visibility follows edges; person nodes public within tenant for mid-market
        allowed_group_ids: groups,
        acl_version: ver,
    }
}

fn map_pull_request(event: &V1CanonicalEvent) -> GraphMutation {
    let (is_private, groups, ver) = acl_fields(event);
    let repo_res = if event.parent_resource_id.is_empty() {
        event
            .resource_id
            .split('/')
            .take(2)
            .collect::<Vec<_>>()
            .join("/")
    } else {
        event.parent_resource_id.clone()
    };
    let pr_res = if event.resource_id.is_empty() {
        format!("{repo_res}/pr/unknown")
    } else {
        event.resource_id.clone()
    };

    let person = person_node(event);
    let repo = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: repo_node_id(&repo_res),
        node_type: "Repo".into(),
        display_name: repo_res.clone(),
        resource_id: repo_res.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let title = event
        .attributes
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let pr = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: pr_node_id(&pr_res),
        node_type: "PullRequest".into(),
        display_name: if title.is_empty() {
            pr_res.clone()
        } else {
            title
        },
        resource_id: pr_res.clone(),
        properties: event.attributes.clone(),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };

    let authored = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: edge_id(
            &event.tenant_id,
            "AUTHORED",
            &person.node_id,
            &pr.node_id,
            &event.event_id,
        ),
        edge_type: "AUTHORED".into(),
        from_node_id: person.node_id.clone(),
        to_node_id: pr.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let belongs = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: stable_edge_id(&event.tenant_id, "BELONGS_TO", &pr.node_id, &repo.node_id),
        edge_type: "BELONGS_TO".into(),
        from_node_id: pr.node_id.clone(),
        to_node_id: repo.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };

    let lifecycle = if event.event_type.contains("merged") {
        "MERGED"
    } else if event.event_type.contains("closed") {
        "CLOSED"
    } else if event.event_type.contains("reopened") || event.event_type.contains("opened") {
        "OPEN"
    } else {
        "OPEN"
    };

    let state = EntityState {
        tenant_id: event.tenant_id.clone(),
        node_id: pr.node_id.clone(),
        state_key: "lifecycle".into(),
        state_value: lifecycle.into(),
        as_of: event.event_timestamp,
        event_id: event.event_id.clone(),
        is_private,
        allowed_group_ids: groups,
    };

    // Optional BLOCKS from attributes
    let mut edges = vec![authored, belongs];
    if let Some(blocks) = event.attributes.get("blocks").and_then(|v| v.as_array()) {
        for b in blocks {
            if let Some(target) = b.as_str() {
                let to = if target.contains("/pr/") {
                    pr_node_id(target)
                } else {
                    issue_node_id(target)
                };
                edges.push(GraphEdge {
                    tenant_id: event.tenant_id.clone(),
                    edge_id: edge_id(
                        &event.tenant_id,
                        "BLOCKS",
                        &pr.node_id,
                        &to,
                        &event.event_id,
                    ),
                    edge_type: "BLOCKS".into(),
                    from_node_id: pr.node_id.clone(),
                    to_node_id: to,
                    valid_from: event.event_timestamp,
                    valid_to: None,
                    event_id: event.event_id.clone(),
                    properties: json!({}),
                    is_private,
                    allowed_group_ids: event.acl.allowed_group_ids.clone(),
                    acl_version: ver,
                });
            }
        }
    }

    GraphMutation {
        nodes: vec![person, repo, pr],
        edges,
        states: vec![state],
    }
}

/// Extract assignee identity from attributes (GitHub/Jira/Linear shapes).
fn extract_assignee_key(event: &V1CanonicalEvent) -> Option<String> {
    let attrs = &event.attributes;
    // Nested object: { "assignee": { "id" | "login" | "accountId" | "global_user_id": ... } }
    if let Some(obj) = attrs.get("assignee").and_then(|v| v.as_object()) {
        for k in ["global_user_id", "id", "login", "accountId", "account_id", "name"] {
            if let Some(s) = obj.get(k).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    // Flat string fields
    for k in [
        "assignee",
        "assignee_id",
        "assignee_account_id",
        "assignee_login",
        "assignee_global_user_id",
    ] {
        if let Some(s) = attrs.get(k).and_then(|v| v.as_str()) {
            if !s.is_empty() && s != "null" {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn person_node_for_key(
    event: &V1CanonicalEvent,
    key: &str,
    display: Option<&str>,
) -> GraphNode {
    let (_is_private, groups, ver) = acl_fields(event);
    let nid = if key.starts_with("gu_") || key.contains('@') {
        person_node_id(key)
    } else if key.starts_with("person:") {
        key.to_string()
    } else {
        person_from_provider(key)
    };
    GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: nid,
        node_type: "Person".into(),
        display_name: display.unwrap_or(key).to_string(),
        resource_id: key.to_string(),
        properties: json!({ "assignee_key": key }),
        is_private: false,
        allowed_group_ids: groups,
        acl_version: ver,
    }
}

fn map_issue(event: &V1CanonicalEvent) -> GraphMutation {
    let (is_private, groups, ver) = acl_fields(event);
    let parent = if event.parent_resource_id.is_empty() {
        "project".into()
    } else {
        event.parent_resource_id.clone()
    };
    let res = if event.resource_id.is_empty() {
        format!("{parent}/issue/unknown")
    } else {
        event.resource_id.clone()
    };
    let person = person_node(event);
    let issue = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: issue_node_id(&res),
        node_type: "Issue".into(),
        display_name: event
            .attributes
            .get("summary")
            .or_else(|| event.attributes.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or(&res)
            .to_string(),
        resource_id: res.clone(),
        properties: event.attributes.clone(),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let authored = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: edge_id(
            &event.tenant_id,
            "AUTHORED",
            &person.node_id,
            &issue.node_id,
            &event.event_id,
        ),
        edge_type: "AUTHORED".into(),
        from_node_id: person.node_id.clone(),
        to_node_id: issue.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let status = event
        .attributes
        .get("status")
        .or_else(|| event.attributes.get("state"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let state = EntityState {
        tenant_id: event.tenant_id.clone(),
        node_id: issue.node_id.clone(),
        state_key: "status".into(),
        state_value: status,
        as_of: event.event_timestamp,
        event_id: event.event_id.clone(),
        is_private,
        allowed_group_ids: groups.clone(),
    };

    let mut nodes = vec![person, issue.clone()];
    let mut edges = vec![authored];

    // ASSIGNED_TO: Issue → Person when assignee present or event is assign-related.
    // Spec: Ticket → Person; task notes Person←Issue.
    let et_l = event.event_type.to_ascii_lowercase();
    let is_assign_event = et_l.contains("assign");
    if let Some(akey) = extract_assignee_key(event) {
        let assignee = person_node_for_key(event, &akey, None);
        let assigned = GraphEdge {
            tenant_id: event.tenant_id.clone(),
            edge_id: stable_edge_id(
                &event.tenant_id,
                "ASSIGNED_TO",
                &issue.node_id,
                &assignee.node_id,
            ),
            edge_type: "ASSIGNED_TO".into(),
            from_node_id: issue.node_id.clone(),
            to_node_id: assignee.node_id.clone(),
            valid_from: event.event_timestamp,
            valid_to: None,
            event_id: event.event_id.clone(),
            properties: json!({ "assignee_key": akey }),
            is_private,
            allowed_group_ids: groups.clone(),
            acl_version: ver,
        };
        nodes.push(assignee);
        edges.push(assigned);
    } else if is_assign_event {
        // jira assign / issue.assigned without structured assignee → actor
        let assignee = person_node(event);
        let assigned = GraphEdge {
            tenant_id: event.tenant_id.clone(),
            edge_id: stable_edge_id(
                &event.tenant_id,
                "ASSIGNED_TO",
                &issue.node_id,
                &assignee.node_id,
            ),
            edge_type: "ASSIGNED_TO".into(),
            from_node_id: issue.node_id.clone(),
            to_node_id: assignee.node_id.clone(),
            valid_from: event.event_timestamp,
            valid_to: None,
            event_id: event.event_id.clone(),
            properties: json!({}),
            is_private,
            allowed_group_ids: groups,
            acl_version: ver,
        };
        edges.push(assigned);
    }

    GraphMutation {
        nodes,
        edges,
        states: vec![state],
    }
}

/// MEMBER_OF: Person → Team from identity / member events.
fn map_team_membership(event: &V1CanonicalEvent) -> GraphMutation {
    let (is_private, groups, ver) = acl_fields(event);
    let team = event
        .attributes
        .get("team")
        .or_else(|| event.attributes.get("group_name"))
        .or_else(|| event.attributes.get("group_id"))
        .or_else(|| event.attributes.get("channel"))
        .or_else(|| event.attributes.get("org"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if team.is_empty() {
        return GraphMutation::default();
    }
    let et = event.event_type.to_ascii_lowercase();
    // Only materialize MEMBER_OF on join/add (not remove).
    if et.contains("remove") || et.contains("left") || et.contains("deleted") {
        return GraphMutation::default();
    }
    let person = person_node(event);
    let team_node = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: team_node_id(team),
        node_type: "Team".into(),
        display_name: team.to_string(),
        resource_id: team.to_string(),
        properties: json!({}),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let edge = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: stable_edge_id(
            &event.tenant_id,
            "MEMBER_OF",
            &person.node_id,
            &team_node.node_id,
        ),
        edge_type: "MEMBER_OF".into(),
        from_node_id: person.node_id.clone(),
        to_node_id: team_node.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({ "team": team }),
        is_private,
        allowed_group_ids: groups,
        acl_version: ver,
    };
    GraphMutation {
        nodes: vec![person, team_node],
        edges: vec![edge],
        states: vec![],
    }
}

fn map_push(event: &V1CanonicalEvent) -> GraphMutation {
    let (is_private, groups, ver) = acl_fields(event);
    let repo_res = event.parent_resource_id.clone();
    if repo_res.is_empty() {
        return GraphMutation::default();
    }
    let person = person_node(event);
    let repo = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: repo_node_id(&repo_res),
        node_type: "Repo".into(),
        display_name: repo_res.clone(),
        resource_id: repo_res.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let mut nodes = vec![person.clone(), repo];
    let mut edges = Vec::new();
    if let Some(commits) = event.attributes.get("commits").and_then(|c| c.as_array()) {
        for c in commits.iter().take(20) {
            let sha = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if sha.is_empty() {
                continue;
            }
            let cid = commit_node_id(&repo_res, sha);
            nodes.push(GraphNode {
                tenant_id: event.tenant_id.clone(),
                node_id: cid.clone(),
                node_type: "Commit".into(),
                display_name: sha.chars().take(7).collect(),
                resource_id: sha.to_string(),
                properties: json!({
                    "message": c.get("message").and_then(|v| v.as_str()).unwrap_or("")
                }),
                is_private,
                allowed_group_ids: groups.clone(),
                acl_version: ver,
            });
            edges.push(GraphEdge {
                tenant_id: event.tenant_id.clone(),
                edge_id: edge_id(
                    &event.tenant_id,
                    "AUTHORED",
                    &person.node_id,
                    &cid,
                    &format!("{}:{}", event.event_id, sha),
                ),
                edge_type: "AUTHORED".into(),
                from_node_id: person.node_id.clone(),
                to_node_id: cid,
                valid_from: event.event_timestamp,
                valid_to: None,
                event_id: event.event_id.clone(),
                properties: json!({}),
                is_private,
                allowed_group_ids: groups.clone(),
                acl_version: ver,
            });
        }
    }
    GraphMutation {
        nodes,
        edges,
        states: vec![],
    }
}

fn map_communication(event: &V1CanonicalEvent) -> GraphMutation {
    let (is_private, groups, ver) = acl_fields(event);
    let channel = event
        .attributes
        .get("channel")
        .or_else(|| event.attributes.get("conversation_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let person = person_node(event);
    let ch = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: channel_node_id(channel),
        node_type: "Channel".into(),
        display_name: channel.to_string(),
        resource_id: channel.to_string(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };
    let edge = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: edge_id(
            &event.tenant_id,
            "DISCUSSED_IN",
            &person.node_id,
            &ch.node_id,
            &event.event_id,
        ),
        edge_type: "DISCUSSED_IN".into(),
        from_node_id: person.node_id.clone(),
        to_node_id: ch.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({
            "preview": event.attributes.get("text_preview").cloned().unwrap_or(json!(""))
        }),
        is_private,
        allowed_group_ids: groups,
        acl_version: ver,
    };
    GraphMutation {
        nodes: vec![person, ch],
        edges: vec![edge],
        states: vec![],
    }
}

fn map_blocker_hint(event: &V1CanonicalEvent) -> GraphMutation {
    // Generic: if attributes have from/to resource ids
    let from = event
        .attributes
        .get("from_resource_id")
        .and_then(|v| v.as_str());
    let to = event
        .attributes
        .get("to_resource_id")
        .and_then(|v| v.as_str());
    if let (Some(f), Some(t)) = (from, to) {
        let (is_private, groups, ver) = acl_fields(event);
        let from_n = pr_node_id(f);
        let to_n = pr_node_id(t);
        return GraphMutation {
            nodes: vec![],
            edges: vec![GraphEdge {
                tenant_id: event.tenant_id.clone(),
                edge_id: edge_id(&event.tenant_id, "BLOCKS", &from_n, &to_n, &event.event_id),
                edge_type: "BLOCKS".into(),
                from_node_id: from_n,
                to_node_id: to_n,
                valid_from: event.event_timestamp,
                valid_to: None,
                event_id: event.event_id.clone(),
                properties: json!({}),
                is_private,
                allowed_group_ids: groups,
                acl_version: ver,
            }],
            states: vec![],
        };
    }
    GraphMutation::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::membership::InMemoryMembership;
    use crate::store::InMemoryGraphStore;
    use crate::v1_event::{V1Actor, V1Acl};
    use chrono::{DateTime, Utc};

    fn sample_pr(action: &str, ts: &str, private: bool) -> V1CanonicalEvent {
        V1CanonicalEvent {
            event_id: format!("evt-{action}"),
            tenant_id: "ten".into(),
            provider: "github".into(),
            category: "code".into(),
            event_type: format!("pull_request.{action}"),
            event_timestamp: DateTime::parse_from_rfc3339(ts)
                .unwrap()
                .with_timezone(&Utc),
            ingested_at: Utc::now(),
            actor: V1Actor {
                global_user_id: "gu_alice".into(),
                provider_user_id: "42".into(),
                email: "a@x.com".into(),
                display_name: "Alice".into(),
            },
            acl: V1Acl {
                tenant_id: "ten".into(),
                allowed_group_ids: vec!["grp_eng".into()],
                is_private: private,
                acl_version: 1,
            },
            resource_id: "acme/app/pr/7".into(),
            parent_resource_id: "acme/app".into(),
            attributes: json!({"title": "Feat", "state": action}),
            raw_payload_s3_uri: String::new(),
            event_sequence_number: 1,
        }
    }

    #[tokio::test]
    async fn projects_pr_and_state() {
        let store = InMemoryGraphStore::new();
        let mem = InMemoryMembership::new();
        let eng = ProjectEngine::new(store.clone(), mem);
        let o = eng.project_event(&sample_pr("opened", "2026-01-01T00:00:00Z", true)).await.unwrap();
        assert_eq!(o.status, ProjectStatus::Applied);
        assert!(store.count_nodes("ten").await.unwrap() >= 3);
    }

    #[tokio::test]
    async fn out_of_order_state() {
        let store = InMemoryGraphStore::new();
        let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
        let mut closed = sample_pr("closed", "2026-01-01T00:00:05Z", false);
        closed.event_id = "c".into();
        let mut opened = sample_pr("opened", "2026-01-01T00:00:00Z", false);
        opened.event_id = "o".into();
        eng.project_event(&closed).await.unwrap();
        eng.project_event(&opened).await.unwrap();
        let ctx = QueryContext {
            tenant_id: "ten".into(),
            global_user_id: "gu_alice".into(),
            group_ids: vec![],
        };
        let st = store
            .get_state(&ctx, &pr_node_id("acme/app/pr/7"), "lifecycle")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(st.state_value, "CLOSED");
    }

    #[test]
    fn pull_request_merged_sets_lifecycle_merged() {
        let m = map_event(&sample_pr("merged", "2026-01-02T00:00:00Z", false));
        let life = m
            .states
            .iter()
            .find(|s| s.state_key == "lifecycle")
            .expect("lifecycle state");
        assert_eq!(life.state_value, "MERGED");
    }

    #[test]
    fn issue_assigned_creates_assigned_to_edge() {
        let mut ev = sample_pr("opened", "2026-01-01T00:00:00Z", false);
        ev.event_id = "iss-assign-1".into();
        ev.event_type = "issue.assigned".into();
        ev.category = "work".into();
        ev.resource_id = "acme/app/issues/9".into();
        ev.parent_resource_id = "acme/app".into();
        ev.attributes = json!({
            "title": "Bug",
            "assignee": "gu_bob",
            "status": "open"
        });
        let m = map_event(&ev);
        assert!(
            m.edges.iter().any(|e| e.edge_type == "ASSIGNED_TO"),
            "expected ASSIGNED_TO edge, got {:?}",
            m.edges.iter().map(|e| &e.edge_type).collect::<Vec<_>>()
        );
        let edge = m
            .edges
            .iter()
            .find(|e| e.edge_type == "ASSIGNED_TO")
            .unwrap();
        assert_eq!(edge.from_node_id, issue_node_id("acme/app/issues/9"));
        assert_eq!(edge.to_node_id, person_node_id("gu_bob"));
        assert!(m.nodes.iter().any(|n| n.node_type == "Issue"));
        assert!(m.nodes.iter().any(|n| n.node_id == person_node_id("gu_bob")));
    }

    #[test]
    fn jira_assign_with_assignee_account_id() {
        let mut ev = sample_pr("opened", "2026-01-01T00:00:00Z", false);
        ev.event_id = "jira-1".into();
        ev.event_type = "jira:issue_updated".into();
        ev.provider = "jira".into();
        ev.resource_id = "PROJ-12".into();
        ev.attributes = json!({
            "summary": "Do thing",
            "assignee_account_id": "jira-acc-99",
            "status": "In Progress"
        });
        let m = map_event(&ev);
        let edge = m
            .edges
            .iter()
            .find(|e| e.edge_type == "ASSIGNED_TO")
            .expect("ASSIGNED_TO");
        assert_eq!(edge.to_node_id, person_from_provider("jira-acc-99"));
    }

    #[test]
    fn identity_member_creates_member_of_edge() {
        let mut ev = sample_pr("opened", "2026-01-01T00:00:00Z", false);
        ev.event_id = "mem-1".into();
        ev.event_type = "identity.team.member_added".into();
        ev.category = "identity".into();
        ev.resource_id = "team/eng".into();
        ev.attributes = json!({
            "team": "eng",
            "member": "gu_alice"
        });
        let m = map_event(&ev);
        assert!(
            m.edges.iter().any(|e| e.edge_type == "MEMBER_OF"),
            "expected MEMBER_OF"
        );
        let edge = m.edges.iter().find(|e| e.edge_type == "MEMBER_OF").unwrap();
        assert_eq!(edge.from_node_id, person_node_id("gu_alice"));
        assert_eq!(edge.to_node_id, team_node_id("eng"));
        assert!(m.nodes.iter().any(|n| n.node_type == "Team"));
    }
}
