#!/usr/bin/env bash
# Integration bridge: Vertical 1 (:18080) → Vertical 2 (:18082) via HTTP project.
#
# Assumes:
#   - V1 telemetry-query / ingestion API on V1_BASE (default http://127.0.0.1:18080)
#   - V2 graph-api on V2_BASE (default http://127.0.0.1:18082)
#
# Flow:
#   1) Health both services
#   2) Seed V2 membership for a test user
#   3) Project a synthetic V1-shaped PR event into V2 (or pull latest V1 event if available)
#   4) Query neighborhood / path to prove graph materialization
#
# For bus-backed projection use graph-projector (Redpanda events.raw + events.acl).
set -euo pipefail

V1_BASE="${V1_BASE:-http://127.0.0.1:18080}"
V2_BASE="${V2_BASE:-http://127.0.0.1:18082}"
TENANT="${TENANT_ID:-ten_bridge}"
USER_ID="${USER_ID:-gu_alice}"
GROUP_ID="${GROUP_ID:-grp_eng}"

echo "== V1 health ($V1_BASE) =="
if curl -sf --max-time 3 "$V1_BASE/healthz" >/tmp/v1_health.json 2>/dev/null \
  || curl -sf --max-time 3 "$V1_BASE/health" >/tmp/v1_health.json 2>/dev/null; then
  cat /tmp/v1_health.json; echo
  V1_UP=1
else
  echo "(V1 not reachable — continuing with synthetic event only)"
  V1_UP=0
fi

echo "== V2 health ($V2_BASE) =="
curl -sf --max-time 3 "$V2_BASE/healthz" | tee /tmp/v2_health.json
echo

echo "== seed V2 user membership =="
curl -sf -X POST "$V2_BASE/v2/tenants/$TENANT/users" \
  -H 'content-type: application/json' \
  -d "{\"global_user_id\":\"$USER_ID\",\"groups\":[\"$GROUP_ID\"]}"
echo

EVENT_JSON=""
if [[ "$V1_UP" == "1" ]]; then
  echo "== try pull latest V1 event for tenant =="
  # Best-effort: V1 list events if the endpoint exists
  if curl -sf --max-time 5 \
    "$V1_BASE/v1/tenants/$TENANT/events?user_id=$USER_ID&limit=1" \
    -o /tmp/v1_events.json 2>/dev/null; then
    if command -v jq >/dev/null 2>&1; then
      EVENT_JSON=$(jq -c '.events[0] // .[0] // empty' /tmp/v1_events.json 2>/dev/null || true)
    fi
  fi
fi

if [[ -z "${EVENT_JSON}" || "${EVENT_JSON}" == "null" ]]; then
  echo "== project synthetic V1-shaped PR (bridge demo) =="
  EVENT_ID="bridge-pr-$(date +%s)"
  EVENT_JSON=$(cat <<EOF
{
  "event_id": "$EVENT_ID",
  "tenant_id": "$TENANT",
  "provider": "github",
  "category": "code",
  "event_type": "pull_request.opened",
  "event_timestamp": "2026-01-15T12:00:00Z",
  "ingested_at": "2026-01-15T12:00:01Z",
  "actor": {
    "global_user_id": "$USER_ID",
    "provider_user_id": "42",
    "email": "alice@example.com",
    "display_name": "Alice"
  },
  "acl": {
    "tenant_id": "$TENANT",
    "allowed_group_ids": ["$GROUP_ID"],
    "is_private": true,
    "acl_version": 1
  },
  "resource_id": "acme/bridge/pr/1",
  "parent_resource_id": "acme/bridge",
  "attributes": {"title": "V1→V2 bridge"},
  "raw_payload_s3_uri": "",
  "event_sequence_number": 1
}
EOF
)
else
  echo "== project V1 event into V2 =="
fi

echo "$EVENT_JSON" | curl -sf -X POST "$V2_BASE/v2/project" \
  -H 'content-type: application/json' \
  -d @- | tee /tmp/v2_project.json
echo

PR_NODE="pr:acme/bridge/pr/1"
PERSON_NODE="person:$USER_ID"
REPO_NODE="repo:acme/bridge"

# If we pulled a real V1 event, derive node ids best-effort
if command -v jq >/dev/null 2>&1 && [[ -n "${EVENT_JSON}" ]]; then
  RID=$(echo "$EVENT_JSON" | jq -r '.resource_id // empty' 2>/dev/null || true)
  if [[ -n "$RID" && "$RID" == *"/pr/"* ]]; then
    PR_NODE="pr:$RID"
  fi
  PARENT=$(echo "$EVENT_JSON" | jq -r '.parent_resource_id // empty' 2>/dev/null || true)
  if [[ -n "$PARENT" ]]; then
    REPO_NODE="repo:$PARENT"
  fi
fi

echo "== neighborhood from person (hops=2) =="
curl -sf "$V2_BASE/v2/tenants/$TENANT/neighborhood?user_id=$USER_ID&node_id=$(python3 -c "import urllib.parse; print(urllib.parse.quote('$PERSON_NODE', safe=''))" 2>/dev/null || echo "$PERSON_NODE")&hops=2" \
  | tee /tmp/v2_nb.json
echo

echo "== path person → repo (if public path via edges) =="
# URL-encode node ids
enc() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1], safe=''))" "$1"
  else
    echo "$1" | sed 's/:/%3A/g; s/\//%2F/g'
  fi
}
PATH_URL="$V2_BASE/v2/tenants/$TENANT/path?user_id=$USER_ID&from=$(enc "$PERSON_NODE")&to=$(enc "$REPO_NODE")&max_hops=3"
code=$(curl -s -o /tmp/v2_path.json -w "%{http_code}" "$PATH_URL")
echo "http=$code body=$(cat /tmp/v2_path.json)"
# Path may be empty if ACL hides edges — not a hard failure when V1-only event used different ids

echo "== stats =="
curl -sf "$V2_BASE/v2/tenants/$TENANT/stats" | tee /tmp/v2_stats.json
echo

echo "== metrics =="
curl -sf "$V2_BASE/metrics" | tee /tmp/v2_metrics.json
echo

# Hard check: project outcome applied/duplicate and stats show nodes
if command -v jq >/dev/null 2>&1; then
  status=$(jq -r '.status // empty' /tmp/v2_project.json)
  nodes=$(jq -r '.nodes // 0' /tmp/v2_stats.json)
  echo "project_status=$status nodes=$nodes"
  # ProjectStatus serializes snake_case: applied | duplicate | skipped
  case "$status" in
    applied|duplicate|skipped|Applied|Duplicate|Skipped) ;;
    *) echo "FAIL: unexpected project status: $status"; exit 1 ;;
  esac
  if [[ "${nodes:-0}" -lt 1 ]]; then
    echo "FAIL: expected graph nodes after project"
    exit 1
  fi
fi

echo "INTEGRATION V1→V2 BRIDGE OK"
echo "  (For continuous bus projection: KAFKA_TOPICS=events.raw,events.acl cargo run -p graph-projector)"
