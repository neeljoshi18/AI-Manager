use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_zendesk(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    // Zendesk triggers often wrap ticket fields at top level or under "detail".
    let detail = v.get("detail").or_else(|| v.get("ticket")).unwrap_or(v);
    let ticket_id = i64_field(detail, "id")
        .or_else(|| {
            str_field(detail, "id")
                .and_then(|s| s.parse().ok())
        })
        .or_else(|| i64_field(v, "id"))
        .unwrap_or(0);
    let event_type = str_field(v, "type")
        .or_else(|| str_field(v, "event_type"))
        .unwrap_or("ticket.updated");
    let actor = detail
        .get("requester")
        .or_else(|| detail.get("assignee"))
        .or_else(|| v.get("actor"))
        .map(|u| actor_from_user(u, &ctx.actor_global_user_id))
        .unwrap_or_default();
    let ts = detail
        .get("updated_at")
        .or_else(|| detail.get("created_at"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::WorkItem,
        event_type,
        ts,
        actor,
        resource_id(&["zendesk", "ticket", &ticket_id.to_string()]),
        "zendesk".into(),
        json!({
            "ticket_id": ticket_id,
            "subject": str_field(detail, "subject").or_else(|| str_field(detail, "title")).unwrap_or(""),
            "status": str_field(detail, "status").unwrap_or(""),
            "priority": str_field(detail, "priority").unwrap_or(""),
            "assignee_id": i64_field(detail, "assignee_id").unwrap_or(0),
            "requester_id": i64_field(detail, "requester_id").unwrap_or(0),
            "tags": detail.get("tags").cloned().unwrap_or(json!([])),
        }),
    ))
}
