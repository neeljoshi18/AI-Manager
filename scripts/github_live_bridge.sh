#!/usr/bin/env bash
# Poll V1 events → project into V2 → compile V3 twin → Slack DM (when twin-api has egress).
# For local embedded stacks where Redpanda projector is not running.
set -euo pipefail

V1="${V1_BASE_URL:-http://127.0.0.1:18080}"
V2="${V2_BASE_URL:-http://127.0.0.1:18082}"
V3="${V3_BASE_URL:-http://127.0.0.1:18083}"
TENANT="${TENANT_ID:-ten_github}"
USER_ID="${USER_ID:-}"   # optional; will use event actor if empty
SLACK_USER="${SLACK_USER_ID:-U0APK7W1X99}"
CHANNEL="${SLACK_CHANNEL_ID:-C0APN754MQV}"
STATE_FILE="${STATE_FILE:-/tmp/ai_manager_bridge_seen_${TENANT}.txt}"
POLL="${POLL_SECS:-3}"

touch "$STATE_FILE"
echo "bridge: V1=$V1 V2=$V2 V3=$V3 tenant=$TENANT (poll ${POLL}s)"
echo "seen file: $STATE_FILE"

seen() { grep -qxF "$1" "$STATE_FILE" 2>/dev/null; }
mark() { echo "$1" >> "$STATE_FILE"; }

while true; do
  # List recent events for a known user if set; else try gu_alice and any from seed
  for CAND in ${USER_ID:-} gu_alice gu_bridge; do
    [[ -z "$CAND" ]] && continue
    EVENTS=$(curl -sf --max-time 5 "$V1/v1/tenants/$TENANT/events?user_id=$CAND&limit=20" 2>/dev/null || echo '{}')
    python3 - "$EVENTS" "$TENANT" "$V2" "$V3" "$SLACK_USER" "$CHANNEL" "$STATE_FILE" <<'PY' || true
import json, sys, urllib.request, os, subprocess

raw, tenant, v2, v3, slack_user, channel, state_file = sys.argv[1:8]
try:
    data = json.loads(raw)
except Exception:
    sys.exit(0)
events = data.get("events") or data if isinstance(data, list) else []
if isinstance(data, dict) and "events" not in data:
    # try common shapes
    for k in ("items", "data"):
        if isinstance(data.get(k), list):
            events = data[k]
            break

seen = set()
try:
    seen = set(open(state_file).read().splitlines())
except Exception:
    pass

def post(url, obj):
    req = urllib.request.Request(
        url,
        data=json.dumps(obj).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        return r.read()

for ev in events:
    if not isinstance(ev, dict):
        continue
    eid = ev.get("event_id") or ev.get("id")
    if not eid or eid in seen:
        continue
    # Normalize to V2 project shape if already canonical-like
    body = {
        "event_id": eid,
        "tenant_id": ev.get("tenant_id") or tenant,
        "provider": ev.get("provider") or "github",
        "category": ev.get("category") or "code",
        "event_type": ev.get("event_type") or "pull_request.opened",
        "event_timestamp": ev.get("event_timestamp") or ev.get("timestamp") or "2026-07-22T00:00:00Z",
        "ingested_at": ev.get("ingested_at") or "2026-07-22T00:00:01Z",
        "actor": ev.get("actor") or {
            "global_user_id": "gu_alice",
            "provider_user_id": "0",
            "display_name": "Alice",
        },
        "acl": ev.get("acl") or {
            "tenant_id": tenant,
            "allowed_group_ids": ["grp_eng"],
            "is_private": False,
            "acl_version": 1,
        },
        "resource_id": ev.get("resource_id") or "",
        "parent_resource_id": ev.get("parent_resource_id") or "",
        "attributes": ev.get("attributes") or {},
    }
    gu = (body.get("actor") or {}).get("global_user_id") or "gu_alice"
    try:
        # seed membership
        post(f"{v2}/v2/tenants/{tenant}/users", {"global_user_id": gu, "groups": ["grp_eng"]})
    except Exception as e:
        print("seed fail", e)
    try:
        post(f"{v2}/v2/project", body)
        print(f"projected {eid} → V2")
    except Exception as e:
        print(f"project fail {eid}: {e}")
        continue

    twin_id = f"twin:person:{gu}"
    try:
        post(f"{v3}/v3/tenants/{tenant}/twins", {
            "twin_kind": "person",
            "subject_id": gu,
            "display_name": (body.get("actor") or {}).get("display_name") or gu,
            "shadow_until": None,
            "high_auto_publish": False,
            "channel_id": channel,
            "slack_user_id": slack_user,
        })
        # clear fixture so V3 reads live V2
        post(f"{v3}/v3/tenants/{tenant}/fixtures", {
            "global_user_id": gu,
            "view": {"nodes": [], "edges": [], "states": [], "blockers": []},
        })
        req = urllib.request.Request(
            f"{v3}/v3/tenants/{tenant}/twins/{twin_id}/compile",
            data=json.dumps({"skip_shadow": True}).encode(),
            headers={"content-type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            out = json.loads(r.read().decode())
        items = len((out.get("ledger") or {}).get("items") or [])
        dm = (out.get("draft") or {}).get("slack_dm_ts")
        print(f"compiled {eid}: items={items} dm_ts={dm} status={(out.get('draft') or {}).get('status')}")
    except Exception as e:
        print(f"compile fail {eid}: {e}")

    with open(state_file, "a") as f:
        f.write(eid + "\n")
    seen.add(eid)
PY
  done
  sleep "$POLL"
done
