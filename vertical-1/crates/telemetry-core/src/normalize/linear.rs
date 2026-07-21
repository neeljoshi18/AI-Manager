use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_linear(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let action = str_field(v, "action").or_else(|| str_field(v, "type")).unwrap_or("update");
    let data = v.get("data").unwrap_or(v);
    let type_name = str_field(v, "type")
        .or_else(|| str_field(data, "type"))
        .unwrap_or("Issue");

    if type_name.eq_ignore_ascii_case("User") || action.contains("member") {
        return normalize_identity(v, ctx, action);
    }

    let identifier = str_field(data, "identifier")
        .or_else(|| str_field(data, "id"))
        .unwrap_or("unknown");
    let team_key = nested_str(data, &["team", "key"])
        .or_else(|| str_field(data, "teamId"))
        .unwrap_or("TEAM");
    let actor = v
        .get("actor")
        .or_else(|| data.get("creator"))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = v
        .get("webhookTimestamp")
        .or_else(|| data.get("updatedAt"))
        .or_else(|| data.get("createdAt"))
        .or_else(|| v.get("createdAt"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    let category = if type_name.eq_ignore_ascii_case("Comment") {
        EventCategory::Communication
    } else {
        EventCategory::WorkItem
    };

    Ok(base_event(
        ctx,
        category,
        &format!("linear.{}.{}", type_name.to_ascii_lowercase(), action),
        ts,
        actor,
        resource_id(&[team_key, identifier]),
        team_key.to_string(),
        json!({
            "identifier": identifier,
            "title": str_field(data, "title").unwrap_or(""),
            "state": nested_str(data, &["state", "name"]).unwrap_or(""),
            "priority": i64_field(data, "priority").unwrap_or(0),
            "url": str_field(data, "url").unwrap_or(""),
            "assignee_id": nested_str(data, &["assignee", "id"]).unwrap_or(""),
        }),
    ))
}

fn normalize_identity(
    v: &Value,
    ctx: &NormalizeContext,
    action: &str,
) -> CoreResult<CanonicalEventRecord> {
    let data = v.get("data").unwrap_or(v);
    let actor = actor_from_user(data, &ctx.actor_global_user_id);
    Ok(base_event(
        ctx,
        EventCategory::Identity,
        &format!("linear.identity.{action}"),
        now_utc(),
        actor,
        format!("linear/user/{}", str_field(data, "id").unwrap_or("unknown")),
        "linear".into(),
        json!({ "email": str_field(data, "email").unwrap_or("") }),
    ))
}
