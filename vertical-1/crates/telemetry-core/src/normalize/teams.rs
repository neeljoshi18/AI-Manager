use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_teams(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    let event_type = str_field(v, "type")
        .or_else(|| nested_str(v, &["eventType"]))
        .unwrap_or("message");
    let activity = str_field(v, "type").unwrap_or(event_type);

    if activity.contains("conversationUpdate")
        || activity.contains("membersAdded")
        || activity.contains("membersRemoved")
    {
        return normalize_identity(v, ctx, activity);
    }

    let conversation_id = nested_str(v, &["conversation", "id"]).unwrap_or("unknown");
    let from_id = nested_str(v, &["from", "id"]).unwrap_or("");
    let from_name = nested_str(v, &["from", "name"]).unwrap_or("");
    let text = str_field(v, "text").unwrap_or("");
    let preview: String = text.chars().take(280).collect();
    let ts = v
        .get("timestamp")
        .or_else(|| v.get("localTimestamp"))
        .map(parse_timestamp)
        .unwrap_or_else(now_utc);

    Ok(base_event(
        ctx,
        EventCategory::Communication,
        &format!("teams.{activity}"),
        ts,
        ActorIdentity {
            global_user_id: ctx.actor_global_user_id.clone(),
            provider_user_id: from_id.to_string(),
            email: String::new(),
            display_name: from_name.to_string(),
        },
        resource_id(&[conversation_id, str_field(v, "id").unwrap_or("0")]),
        conversation_id.to_string(),
        json!({
            "conversation_id": conversation_id,
            "text_preview": preview,
            "channel_id": nested_str(v, &["channelData", "channel", "id"]).unwrap_or(""),
            "team_id": nested_str(v, &["channelData", "team", "id"]).unwrap_or(""),
        }),
    ))
}

fn normalize_identity(
    v: &Value,
    ctx: &NormalizeContext,
    activity: &str,
) -> CoreResult<CanonicalEventRecord> {
    let conversation_id = nested_str(v, &["conversation", "id"]).unwrap_or("unknown");
    Ok(base_event(
        ctx,
        EventCategory::Identity,
        &format!("teams.{activity}"),
        now_utc(),
        ActorIdentity::default(),
        conversation_id.to_string(),
        conversation_id.to_string(),
        json!({
            "members_added": v.get("membersAdded").cloned().unwrap_or(json!([])),
            "members_removed": v.get("membersRemoved").cloned().unwrap_or(json!([])),
        }),
    ))
}
