use super::*;
use crate::model::resource_id;
use serde_json::json;

pub fn normalize_slack(v: &Value, ctx: &NormalizeContext) -> CoreResult<CanonicalEventRecord> {
    // URL verification challenge is handled at the HTTP layer; if we get here, process events.
    if str_field(v, "type") == Some("url_verification") {
        return Err(CoreError::Validation(
            "url_verification must be handled by HTTP layer".into(),
        ));
    }

    let event = v.get("event").unwrap_or(v);
    let event_type = str_field(event, "type").unwrap_or("message");
    let team = str_field(v, "team_id")
        .or_else(|| str_field(event, "team"))
        .unwrap_or("unknown");
    let channel = str_field(event, "channel").unwrap_or("unknown");
    let user = str_field(event, "user").unwrap_or("");
    let ts_str = str_field(event, "ts").or_else(|| str_field(event, "event_ts"));
    let ts = ts_str
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| from_secs(f as i64))
        .unwrap_or_else(now_utc);

    if event_type == "member_joined_channel"
        || event_type == "member_left_channel"
        || event_type == "team_join"
        || event_type == "user_change"
        || event_type == "subteam_members_changed"
    {
        return normalize_identity(event, ctx, event_type, team);
    }

    let text = str_field(event, "text").unwrap_or("");
    let preview: String = text.chars().take(280).collect();
    let thread_ts = str_field(event, "thread_ts").unwrap_or("");

    let actor = ActorIdentity {
        global_user_id: ctx.actor_global_user_id.clone(),
        provider_user_id: user.to_string(),
        email: String::new(),
        display_name: String::new(),
    };

    Ok(base_event(
        ctx,
        EventCategory::Communication,
        &format!("slack.{event_type}"),
        ts,
        actor,
        resource_id(&[team, channel, ts_str.unwrap_or("0")]),
        resource_id(&[team, channel]),
        json!({
            "channel": channel,
            "team_id": team,
            "thread_ts": thread_ts,
            "text_preview": preview,
            "subtype": str_field(event, "subtype").unwrap_or(""),
            // No full message history dump — metadata + short preview only.
        }),
    ))
}

fn normalize_identity(
    event: &Value,
    ctx: &NormalizeContext,
    event_type: &str,
    team: &str,
) -> CoreResult<CanonicalEventRecord> {
    let user = str_field(event, "user")
        .or_else(|| nested_str(event, &["user", "id"]))
        .unwrap_or("");
    Ok(base_event(
        ctx,
        EventCategory::Identity,
        &format!("slack.{event_type}"),
        now_utc(),
        ActorIdentity {
            global_user_id: ctx.actor_global_user_id.clone(),
            provider_user_id: user.to_string(),
            email: String::new(),
            display_name: String::new(),
        },
        resource_id(&[team, "user", user]),
        team.to_string(),
        json!({
            "channel": str_field(event, "channel").unwrap_or(""),
            "user": user,
        }),
    ))
}
