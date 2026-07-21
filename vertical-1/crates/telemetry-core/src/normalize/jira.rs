use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_jira(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let webhook_event = str_field(v, "webhookEvent")
        .or_else(|| str_field(v, "issue_event_type_name"))
        .unwrap_or("jira:event");

    if webhook_event.contains("user") || webhook_event.contains("permission") {
        return normalize_identity(v, ctx, webhook_event);
    }

    let issue = v.get("issue");
    let key = issue
        .and_then(|i| str_field(i, "key"))
        .unwrap_or("UNKNOWN-0");
    let fields = issue.and_then(|i| i.get("fields"));
    let project_key = fields
        .and_then(|f| nested_str(f, &["project", "key"]))
        .unwrap_or("PROJ");
    let actor = v
        .get("user")
        .or_else(|| nested(v, &["comment", "author"]))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();

    let ts = fields
        .and_then(|f| f.get("updated"))
        .or_else(|| fields.and_then(|f| f.get("created")))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    let category = if webhook_event.contains("comment") {
        EventCategory::Communication
    } else {
        EventCategory::WorkItem
    };

    let status = fields
        .and_then(|f| nested_str(f, &["status", "name"]))
        .unwrap_or("");
    let summary = fields.and_then(|f| str_field(f, "summary")).unwrap_or("");
    let issue_type = fields
        .and_then(|f| nested_str(f, &["issuetype", "name"]))
        .unwrap_or("");
    let assignee = fields
        .and_then(|f| nested_str(f, &["assignee", "accountId"]))
        .unwrap_or("");

    // Comment preview only — not full ticket description bodies at scale.
    let comment_preview = nested(v, &["comment"])
        .and_then(|c| str_field(c, "body"))
        .map(|b| b.chars().take(280).collect::<String>())
        .unwrap_or_default();

    Ok(base_event(
        ctx,
        category,
        webhook_event,
        ts,
        actor,
        resource_id(&[project_key, key]),
        project_key.to_string(),
        json!({
            "key": key,
            "summary": summary,
            "status": status,
            "issue_type": issue_type,
            "assignee_account_id": assignee,
            "comment_preview": comment_preview,
            "changelog_items": extract_changelog(v),
        }),
    ))
}

fn extract_changelog(v: &Value) -> Vec<Value> {
    v.get("changelog")
        .and_then(|c| c.get("items"))
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .take(20)
                .map(|item| {
                    json!({
                        "field": str_field(item, "field").unwrap_or(""),
                        "from": str_field(item, "fromString").unwrap_or(""),
                        "to": str_field(item, "toString").unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_identity(
    v: &Value,
    ctx: &NormalizeContext,
    event: &str,
) -> CoreResult<CanonicalEventRecord> {
    let actor = v
        .get("user")
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let resource = format!("jira/identity/{}", actor.provider_user_id);
    Ok(base_event(
        ctx,
        EventCategory::Identity,
        event,
        now_utc(),
        actor,
        resource,
        "jira".into(),
        json!({ "event": event }),
    ))
}
