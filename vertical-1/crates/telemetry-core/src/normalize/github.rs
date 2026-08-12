use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_github(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let action = str_field(v, "action").unwrap_or("");
    let event_name = ctx.event_name.as_deref().unwrap_or("");

    // Identity / membership events
    if event_name == "membership" || event_name == "member" || event_name == "organization" {
        return normalize_github_identity(v, ctx, event_name, action);
    }

    if v.get("pull_request").is_some() || event_name == "pull_request" {
        return normalize_pr(v, ctx, action);
    }
    if v.get("issue").is_some() && v.get("pull_request").is_none() {
        return normalize_issue(v, ctx, action);
    }
    if event_name == "push" || v.get("commits").is_some() {
        return normalize_push(v, ctx);
    }
    if event_name == "create" || (str_field(v, "ref_type") == Some("branch")) {
        return normalize_branch(v, ctx);
    }
    if v.get("comment").is_some() {
        return normalize_comment(v, ctx, action);
    }
    if event_name == "check_run" || v.get("check_run").is_some() {
        return normalize_check(v, ctx, action);
    }

    // Generic fallback — still structured, never stores code blobs.
    let actor = v
        .get("sender")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let ts = v
        .get("repository")
        .and_then(|r| r.get("updated_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::Code,
        &format!(
            "{}.{}",
            if event_name.is_empty() { "github" } else { event_name },
            if action.is_empty() { "event" } else { action }
        ),
        ts,
        actor,
        repo.to_string(),
        repo.to_string(),
        json!({
            "provider_event": event_name,
            "action": action,
            "repository": repo,
        }),
    ))
}

fn normalize_pr(v: &Value, ctx: &NormalizeContext, action: &str) -> CoreResult<CanonicalEventRecord> {
    let pr = v
        .get("pull_request")
        .ok_or_else(|| CoreError::Normalization("missing pull_request".into()))?;
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let number = i64_field(pr, "number").unwrap_or(0);
    let actor = v
        .get("sender")
        .or_else(|| pr.get("user"))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = pr
        .get("updated_at")
        .or_else(|| pr.get("created_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);
    let is_private = nested(v, &["repository"])
        .and_then(|r| bool_field(r, "private"))
        .unwrap_or(ctx.is_private);

    let mut event = base_event(
        ctx,
        EventCategory::Code,
        &format!("pull_request.{}", if action.is_empty() { "updated" } else { action }),
        ts,
        actor,
        resource_id(&[repo, "pr", &number.to_string()]),
        repo.to_string(),
        json!({
            "title": str_field(pr, "title").unwrap_or(""),
            "state": str_field(pr, "state").unwrap_or(""),
            "draft": bool_field(pr, "draft").unwrap_or(false),
            "merged": bool_field(pr, "merged").unwrap_or(false),
            "base_ref": nested_str(pr, &["base", "ref"]).unwrap_or(""),
            "head_ref": nested_str(pr, &["head", "ref"]).unwrap_or(""),
            "additions": i64_field(pr, "additions").unwrap_or(0),
            "deletions": i64_field(pr, "deletions").unwrap_or(0),
            "changed_files": i64_field(pr, "changed_files").unwrap_or(0),
            "html_url": str_field(pr, "html_url").unwrap_or(""),
            // Labels + short body for rules_v0 intent classify (FREEZE/BLOCKED/SHIP) — not full patch.
            "labels": pr_label_names(pr),
            "body_preview": pr_body_preview(pr),
            "body": pr_body_preview(pr),
            "mergeable_state": str_field(pr, "mergeable_state").unwrap_or(""),
            "updated_at": str_field(pr, "updated_at").unwrap_or(""),
            // Metadata only — no patch / diff / file contents.
        }),
    );
    event.acl.is_private = is_private;
    Ok(event)
}

/// Label names only (no ids) for intent rules.
fn pr_label_names(pr: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(arr) = pr.get("labels").and_then(|v| v.as_array()) {
        for lab in arr {
            if let Some(name) = lab.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    out.push(name.to_string());
                }
            } else if let Some(s) = lab.as_str() {
                if !s.is_empty() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// Truncate PR body for claim classification (≤280). Never store full markdown dumps.
fn pr_body_preview(pr: &Value) -> String {
    let body = str_field(pr, "body").unwrap_or("");
    body.chars().take(280).collect()
}

fn normalize_issue(
    v: &Value,
    ctx: &NormalizeContext,
    action: &str,
) -> CoreResult<CanonicalEventRecord> {
    let issue = v
        .get("issue")
        .ok_or_else(|| CoreError::Normalization("missing issue".into()))?;
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let number = i64_field(issue, "number").unwrap_or(0);
    let actor = v
        .get("sender")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = issue
        .get("updated_at")
        .or_else(|| issue.get("created_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::WorkItem,
        &format!("issue.{}", if action.is_empty() { "updated" } else { action }),
        ts,
        actor,
        resource_id(&[repo, "issue", &number.to_string()]),
        repo.to_string(),
        json!({
            "title": str_field(issue, "title").unwrap_or(""),
            "state": str_field(issue, "state").unwrap_or(""),
            "html_url": str_field(issue, "html_url").unwrap_or(""),
            "labels": pr_label_names(issue),
            "body_preview": pr_body_preview(issue),
            "body": pr_body_preview(issue),
        }),
    ))
}

fn normalize_push(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let actor = v
        .get("sender")
        .or_else(|| v.get("pusher"))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let commits = v
        .get("commits")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    // Collect commit SHAs + messages only (no file patches).
    let commit_meta: Vec<Value> = v
        .get("commits")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .take(50)
                .map(|c| {
                    json!({
                        "id": str_field(c, "id").unwrap_or(""),
                        "message": str_field(c, "message").unwrap_or(""),
                        "timestamp": str_field(c, "timestamp").unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let ts = commit_meta
        .last()
        .and_then(|c| c.get("timestamp"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::Code,
        "push",
        ts,
        actor,
        resource_id(&[repo, "ref", str_field(v, "ref").unwrap_or("heads/unknown")]),
        repo.to_string(),
        json!({
            "ref": str_field(v, "ref").unwrap_or(""),
            "before": str_field(v, "before").unwrap_or(""),
            "after": str_field(v, "after").unwrap_or(""),
            "commit_count": commits,
            "commits": commit_meta,
            "forced": bool_field(v, "forced").unwrap_or(false),
        }),
    ))
}

fn normalize_branch(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let actor = v
        .get("sender")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let r#ref = str_field(v, "ref").unwrap_or("unknown");
    Ok(base_event(
        ctx,
        EventCategory::Code,
        "branch.created",
        now_utc(),
        actor,
        resource_id(&[repo, "branch", r#ref]),
        repo.to_string(),
        json!({ "ref": r#ref, "ref_type": str_field(v, "ref_type").unwrap_or("branch") }),
    ))
}

fn normalize_comment(
    v: &Value,
    ctx: &NormalizeContext,
    action: &str,
) -> CoreResult<CanonicalEventRecord> {
    let comment = v.get("comment").unwrap_or(v);
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    let actor = comment
        .get("user")
        .or_else(|| v.get("sender"))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = comment
        .get("updated_at")
        .or_else(|| comment.get("created_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);
    // Communication metadata only — body truncated to avoid storing large content.
    let body = str_field(comment, "body").unwrap_or("");
    let body_preview: String = body.chars().take(280).collect();

    Ok(base_event(
        ctx,
        EventCategory::Communication,
        &format!("comment.{}", if action.is_empty() { "created" } else { action }),
        ts,
        actor,
        resource_id(&[
            repo,
            "comment",
            &i64_field(comment, "id").unwrap_or(0).to_string(),
        ]),
        repo.to_string(),
        json!({
            "body_preview": body_preview,
            "html_url": str_field(comment, "html_url").unwrap_or(""),
        }),
    ))
}

fn normalize_check(
    v: &Value,
    ctx: &NormalizeContext,
    action: &str,
) -> CoreResult<CanonicalEventRecord> {
    let check = v.get("check_run").unwrap_or(v);
    let repo = nested_str(v, &["repository", "full_name"]).unwrap_or("unknown");
    Ok(base_event(
        ctx,
        EventCategory::Code,
        &format!("check_run.{}", if action.is_empty() { "completed" } else { action }),
        now_utc(),
        ActorIdentity::default(),
        resource_id(&[
            repo,
            "check",
            &i64_field(check, "id").unwrap_or(0).to_string(),
        ]),
        repo.to_string(),
        json!({
            "name": str_field(check, "name").unwrap_or(""),
            "status": str_field(check, "status").unwrap_or(""),
            "conclusion": str_field(check, "conclusion").unwrap_or(""),
        }),
    ))
}

fn normalize_github_identity(
    v: &Value,
    ctx: &NormalizeContext,
    event_name: &str,
    action: &str,
) -> CoreResult<CanonicalEventRecord> {
    let actor = v
        .get("sender")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let member = v
        .get("member")
        .or_else(|| nested(v, &["membership", "user"]));
    let team = nested_str(v, &["team", "slug"])
        .or_else(|| nested_str(v, &["team", "name"]))
        .unwrap_or("");
    let member_login = member.and_then(|m| str_field(m, "login")).unwrap_or("");

    Ok(base_event(
        ctx,
        EventCategory::Identity,
        &format!(
            "identity.{}.{}",
            event_name,
            if action.is_empty() { "changed" } else { action }
        ),
        now_utc(),
        actor,
        resource_id(&["team", team, "member", member_login]),
        format!("team/{team}"),
        json!({
            "team": team,
            "member": member_login,
            "action": action,
            "scope": str_field(v, "scope").unwrap_or(""),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pr_opened() {
        let body = json!({
            "action": "opened",
            "pull_request": {
                "number": 42,
                "title": "Add feature",
                "state": "open",
                "draft": false,
                "merged": false,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "html_url": "https://github.com/acme/app/pull/42",
                "user": { "id": 1, "login": "alice" },
                "base": { "ref": "main" },
                "head": { "ref": "feat" },
                "additions": 10,
                "deletions": 2,
                "changed_files": 1
            },
            "repository": { "full_name": "acme/app", "private": true },
            "sender": { "id": 1, "login": "alice" }
        });
        let ctx = NormalizeContext {
            tenant_id: "ten_1".into(),
            provider: SourceProvider::Github,
            delivery_id: Some("deliv-1".into()),
            event_name: Some("pull_request".into()),
            raw_payload_s3_uri: "s3://b/x".into(),
            default_group_ids: vec!["eng".into()],
            actor_global_user_id: "gu_1".into(),
            acl_version: 1,
            allowed_group_ids: vec!["eng".into()],
            is_private: true,
        };
        let raw = serde_json::to_vec(&body).unwrap();
        let evt = normalize_github(&body, &ctx).unwrap();
        assert_eq!(evt.event_id, "deliv-1");
        assert_eq!(evt.event_type, "pull_request.opened");
        assert_eq!(evt.resource_id, "acme/app/pr/42");
        assert!(evt.acl.is_private);
        let _ = raw;
    }
}
