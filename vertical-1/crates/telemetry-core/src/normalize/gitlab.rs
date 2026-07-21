use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_gitlab(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let object_kind = str_field(v, "object_kind")
        .or_else(|| str_field(v, "event_name"))
        .unwrap_or("event");

    match object_kind {
        "merge_request" => normalize_mr(v, ctx),
        "push" | "tag_push" => normalize_push(v, ctx, object_kind),
        "issue" => normalize_issue(v, ctx),
        "note" | "comment" => normalize_note(v, ctx),
        "pipeline" | "build" => normalize_pipeline(v, ctx, object_kind),
        "member" | "user_add_to_group" | "user_remove_from_group" => {
            normalize_identity(v, ctx, object_kind)
        }
        other => {
            let project = nested_str(v, &["project", "path_with_namespace"])
                .or_else(|| nested_str(v, &["project", "name"]))
                .unwrap_or("unknown");
            let actor = v
                .get("user")
                .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
                .unwrap_or_default();
            Ok(base_event(
                ctx,
                EventCategory::Code,
                &format!("gitlab.{other}"),
                now_utc(),
                actor,
                project.to_string(),
                project.to_string(),
                json!({ "object_kind": other, "project": project }),
            ))
        }
    }
}

fn normalize_mr(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let attrs = v
        .get("object_attributes")
        .ok_or_else(|| CoreError::Normalization("missing object_attributes".into()))?;
    let project = nested_str(v, &["project", "path_with_namespace"]).unwrap_or("unknown");
    let iid = i64_field(attrs, "iid").unwrap_or(0);
    let action = str_field(attrs, "action").unwrap_or("update");
    let actor = v
        .get("user")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = attrs
        .get("updated_at")
        .or_else(|| attrs.get("created_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::Code,
        &format!("merge_request.{action}"),
        ts,
        actor,
        resource_id(&[project, "mr", &iid.to_string()]),
        project.to_string(),
        json!({
            "title": str_field(attrs, "title").unwrap_or(""),
            "state": str_field(attrs, "state").unwrap_or(""),
            "source_branch": str_field(attrs, "source_branch").unwrap_or(""),
            "target_branch": str_field(attrs, "target_branch").unwrap_or(""),
            "url": str_field(attrs, "url").unwrap_or(""),
        }),
    ))
}

fn normalize_push(
    v: &Value,
    ctx: &NormalizeContext,
    kind: &str,
) -> CoreResult<CanonicalEventRecord> {
    let project = nested_str(v, &["project", "path_with_namespace"]).unwrap_or("unknown");
    let actor = ActorIdentity {
        global_user_id: ctx.actor_global_user_id.clone(),
        provider_user_id: i64_field(v, "user_id")
            .map(|n| n.to_string())
            .unwrap_or_default(),
        email: str_field(v, "user_email").unwrap_or("").to_string(),
        display_name: str_field(v, "user_name").unwrap_or("").to_string(),
    };
    let commits = v
        .get("commits")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(base_event(
        ctx,
        EventCategory::Code,
        kind,
        now_utc(),
        actor,
        resource_id(&[project, "ref", str_field(v, "ref").unwrap_or("unknown")]),
        project.to_string(),
        json!({
            "ref": str_field(v, "ref").unwrap_or(""),
            "checkout_sha": str_field(v, "checkout_sha").unwrap_or(""),
            "commit_count": commits,
            "total_commits_count": i64_field(v, "total_commits_count").unwrap_or(0),
        }),
    ))
}

fn normalize_issue(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let attrs = v.get("object_attributes").unwrap_or(v);
    let project = nested_str(v, &["project", "path_with_namespace"]).unwrap_or("unknown");
    let iid = i64_field(attrs, "iid").unwrap_or(0);
    let action = str_field(attrs, "action").unwrap_or("update");
    let actor = v
        .get("user")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = attrs
        .get("updated_at")
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::WorkItem,
        &format!("issue.{action}"),
        ts,
        actor,
        resource_id(&[project, "issue", &iid.to_string()]),
        project.to_string(),
        json!({
            "title": str_field(attrs, "title").unwrap_or(""),
            "state": str_field(attrs, "state").unwrap_or(""),
        }),
    ))
}

fn normalize_note(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let attrs = v.get("object_attributes").unwrap_or(v);
    let project = nested_str(v, &["project", "path_with_namespace"]).unwrap_or("unknown");
    let actor = v
        .get("user")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let body = str_field(attrs, "note").unwrap_or("");
    let preview: String = body.chars().take(280).collect();

    Ok(base_event(
        ctx,
        EventCategory::Communication,
        "note.created",
        now_utc(),
        actor,
        resource_id(&[
            project,
            "note",
            &i64_field(attrs, "id").unwrap_or(0).to_string(),
        ]),
        project.to_string(),
        json!({ "body_preview": preview, "noteable_type": str_field(attrs, "noteable_type").unwrap_or("") }),
    ))
}

fn normalize_pipeline(
    v: &Value,
    ctx: &NormalizeContext,
    kind: &str,
) -> CoreResult<CanonicalEventRecord> {
    let attrs = v.get("object_attributes").unwrap_or(v);
    let project = nested_str(v, &["project", "path_with_namespace"]).unwrap_or("unknown");
    Ok(base_event(
        ctx,
        EventCategory::Code,
        &format!("{kind}.{}", str_field(attrs, "status").unwrap_or("updated")),
        now_utc(),
        ActorIdentity::default(),
        resource_id(&[
            project,
            kind,
            &i64_field(attrs, "id").unwrap_or(0).to_string(),
        ]),
        project.to_string(),
        json!({
            "status": str_field(attrs, "status").unwrap_or(""),
            "ref": str_field(attrs, "ref").unwrap_or(""),
        }),
    ))
}

fn normalize_identity(
    v: &Value,
    ctx: &NormalizeContext,
    kind: &str,
) -> CoreResult<CanonicalEventRecord> {
    let actor = v
        .get("user")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    Ok(base_event(
        ctx,
        EventCategory::Identity,
        &format!("identity.{kind}"),
        now_utc(),
        actor,
        format!("group/{}", str_field(v, "group_name").unwrap_or("unknown")),
        "group".into(),
        json!({
            "group_name": str_field(v, "group_name").unwrap_or(""),
            "user_name": str_field(v, "user_name").unwrap_or(""),
            "group_access": str_field(v, "group_access").unwrap_or(""),
        }),
    ))
}
