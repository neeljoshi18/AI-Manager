#!/usr/bin/env bash
# Golden path: V1 event → V2 project → V3 compile → draft (mock Slack).
# Requires graph-api :18082 and twin-api :18083 (embedded ok). V1 optional.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MONO="$(cd "$ROOT/.." && pwd)"
V2="${V2_BASE_URL:-http://127.0.0.1:18082}"
V3="${V3_BASE_URL:-http://127.0.0.1:18083}"
TENANT="${TENANT_ID:-ten_sew}"
USER="${USER_ID:-gu_alice}"

echo "=== SEW E2E V1→V2→V3 (tenant=$TENANT) ==="

# Prefer in-process battery when services are down
if ! curl -sf "$V3/healthz" >/dev/null 2>&1; then
  echo "twin-api not up on $V3 — running embedded TC-T10 via twin-verify"
  cd "$ROOT"
  cargo run -q -p twin-verify
  echo "SEW E2E OK (embedded verify)"
  exit 0
fi

echo "== V3 health =="
curl -sf "$V3/healthz"
echo

# If V2 is up, project a PR event; else inject V3 fixture
if curl -sf "$V2/healthz" >/dev/null 2>&1; then
  echo "== V2 seed + project PR (simulates V1 canonical event) =="
  curl -sf -X POST "$V2/v2/tenants/$TENANT/users" \
    -H 'content-type: application/json' \
    -d "{\"global_user_id\":\"$USER\",\"groups\":[\"grp_eng\"]}" || true
  echo
  curl -sf -X POST "$V2/v2/project" -H 'content-type: application/json' -d "{
    \"event_id\": \"sew-pr-1\",
    \"tenant_id\": \"$TENANT\",
    \"provider\": \"github\",
    \"category\": \"code\",
    \"event_type\": \"pull_request.opened\",
    \"event_timestamp\": \"2026-07-21T12:00:00Z\",
    \"ingested_at\": \"2026-07-21T12:00:01Z\",
    \"actor\": {\"global_user_id\": \"$USER\", \"provider_user_id\": \"42\", \"display_name\": \"Alice\"},
    \"acl\": {\"tenant_id\": \"$TENANT\", \"allowed_group_ids\": [\"grp_eng\"], \"is_private\": false, \"acl_version\": 1},
    \"resource_id\": \"acme/app/pr/42\",
    \"parent_resource_id\": \"acme/app\",
    \"attributes\": {\"title\": \"Sew E2E PR\"}
  }"
  echo
  echo "== V2 neighborhood (ACL as person) =="
  curl -sf "$V2/v2/tenants/$TENANT/neighborhood?user_id=$USER&node_id=person%3A$USER&hops=2" | head -c 400
  echo
else
  echo "V2 not up — injecting fixture into V3 embedded API"
  curl -sf -X POST "$V3/v3/tenants/$TENANT/fixtures" \
    -H 'content-type: application/json' \
    -d "{
      \"global_user_id\": \"$USER\",
      \"view\": {
        \"nodes\": [
          {\"node_id\":\"person:$USER\",\"node_type\":\"Person\",\"display_name\":\"Alice\",\"resource_id\":\"$USER\",\"properties\":{},\"is_private\":false},
          {\"node_id\":\"pr:acme/app/pr/42\",\"node_type\":\"PullRequest\",\"display_name\":\"Sew\",\"resource_id\":\"acme/app/pr/42\",\"properties\":{\"title\":\"Sew E2E PR\"},\"is_private\":false}
        ],
        \"edges\": [
          {\"edge_id\":\"authored:sew\",\"edge_type\":\"AUTHORED\",\"from_node_id\":\"person:$USER\",\"to_node_id\":\"pr:acme/app/pr/42\",\"event_id\":\"sew-pr-1\",\"properties\":{},\"is_private\":false}
        ],
        \"states\": [
          {\"node_id\":\"pr:acme/app/pr/42\",\"state_key\":\"lifecycle\",\"state_value\":\"OPEN\",\"event_id\":\"sew-pr-1\",\"as_of\":\"2026-07-21T12:00:00Z\"}
        ],
        \"blockers\": []
      }
    }"
  echo
fi

echo "== V3 upsert twin =="
curl -sf -X POST "$V3/v3/tenants/$TENANT/twins" \
  -H 'content-type: application/json' \
  -d "{
    \"twin_kind\": \"person\",
    \"subject_id\": \"$USER\",
    \"display_name\": \"Alice\",
    \"channel_id\": \"C_SEW\",
    \"shadow_until\": null,
    \"high_auto_publish\": false,
    \"slack_user_id\": \"U_SEW\"
  }"
echo

TWIN_ID="twin:person:$USER"
echo "== V3 compile =="
# Production twin-api uses HttpV2GraphSource; embedded uses fixtures.
# When both V2+V3 embedded are used, V3 may not share V2 memory — prefer fixture inject above
# and/or set V2_BASE_URL when twin-api is production-mode against V2.
COMP=$(curl -sf -X POST "$V3/v3/tenants/$TENANT/twins/$TWIN_ID/compile" \
  -H 'content-type: application/json' \
  -d '{"skip_shadow": true}')
echo "$COMP" | head -c 600
echo

DRAFT_STATUS=$(echo "$COMP" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p' | head -1)
LEDGER_ID=$(echo "$COMP" | sed -n 's/.*"ledger_id":"\([^"]*\)".*/\1/p' | head -1)

if [[ -z "$LEDGER_ID" ]]; then
  echo "FAIL: no ledger_id in compile response"
  exit 1
fi

echo "draft_status=$DRAFT_STATUS ledger_id=$LEDGER_ID"
echo "== metrics =="
curl -sf "$V3/metrics"
echo

# Always also run unit battery for TC-T01–T10
echo "== twin-verify battery =="
cd "$ROOT"
cargo run -q -p twin-verify

echo "SEW E2E OK"
