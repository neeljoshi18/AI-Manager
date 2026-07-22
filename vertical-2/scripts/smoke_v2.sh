#!/usr/bin/env bash
# Smoke test against running graph-api (default embedded on :18082)
set -euo pipefail
BASE="${BASE_URL:-http://127.0.0.1:18082}"
TENANT="${TENANT_ID:-ten_smoke}"

echo "== health =="
curl -sf "$BASE/healthz"
echo

echo "== seed alice eng =="
curl -sf -X POST "$BASE/v2/tenants/$TENANT/users" \
  -H 'content-type: application/json' \
  -d '{"global_user_id":"gu_alice","groups":["grp_eng"]}'
echo

echo "== project private PR =="
curl -sf -X POST "$BASE/v2/project" -H 'content-type: application/json' -d "{
  \"event_id\": \"smoke-pr-1\",
  \"tenant_id\": \"$TENANT\",
  \"provider\": \"github\",
  \"category\": \"code\",
  \"event_type\": \"pull_request.opened\",
  \"event_timestamp\": \"2026-01-01T00:00:00Z\",
  \"ingested_at\": \"2026-01-01T00:00:01Z\",
  \"actor\": {\"global_user_id\": \"gu_alice\", \"provider_user_id\": \"42\", \"display_name\": \"Alice\"},
  \"acl\": {\"tenant_id\": \"$TENANT\", \"allowed_group_ids\": [\"grp_eng\"], \"is_private\": true, \"acl_version\": 1},
  \"resource_id\": \"acme/app/pr/7\",
  \"parent_resource_id\": \"acme/app\",
  \"attributes\": {\"title\": \"Smoke\"}
}"
echo

echo "== project issue.assigned (ASSIGNED_TO) =="
curl -sf -X POST "$BASE/v2/project" -H 'content-type: application/json' -d "{
  \"event_id\": \"smoke-iss-1\",
  \"tenant_id\": \"$TENANT\",
  \"provider\": \"github\",
  \"category\": \"work\",
  \"event_type\": \"issue.assigned\",
  \"event_timestamp\": \"2026-01-01T00:01:00Z\",
  \"ingested_at\": \"2026-01-01T00:01:01Z\",
  \"actor\": {\"global_user_id\": \"gu_alice\", \"provider_user_id\": \"42\", \"display_name\": \"Alice\"},
  \"acl\": {\"tenant_id\": \"$TENANT\", \"allowed_group_ids\": [\"grp_eng\"], \"is_private\": false, \"acl_version\": 1},
  \"resource_id\": \"acme/app/issues/3\",
  \"parent_resource_id\": \"acme/app\",
  \"attributes\": {\"title\": \"Bug\", \"assignee\": \"gu_bob\", \"status\": \"open\"}
}"
echo

echo "== project identity member (MEMBER_OF) =="
curl -sf -X POST "$BASE/v2/project" -H 'content-type: application/json' -d "{
  \"event_id\": \"smoke-mem-1\",
  \"tenant_id\": \"$TENANT\",
  \"provider\": \"slack\",
  \"category\": \"identity\",
  \"event_type\": \"identity.team.member_added\",
  \"event_timestamp\": \"2026-01-01T00:02:00Z\",
  \"ingested_at\": \"2026-01-01T00:02:01Z\",
  \"actor\": {\"global_user_id\": \"gu_alice\", \"provider_user_id\": \"42\", \"display_name\": \"Alice\"},
  \"acl\": {\"tenant_id\": \"$TENANT\", \"allowed_group_ids\": [\"grp_eng\"], \"is_private\": false, \"acl_version\": 1},
  \"resource_id\": \"team/eng\",
  \"parent_resource_id\": \"\",
  \"attributes\": {\"team\": \"eng\", \"member\": \"gu_alice\"}
}"
echo

echo "== alice neighborhood =="
curl -sf "$BASE/v2/tenants/$TENANT/neighborhood?user_id=gu_alice&node_id=person%3Agu_alice&hops=2"
echo

echo "== bob (no groups) should not see private PR node =="
curl -sf -X POST "$BASE/v2/tenants/$TENANT/users" \
  -H 'content-type: application/json' \
  -d '{"global_user_id":"gu_bob","groups":[]}'
echo
code=$(curl -s -o /tmp/v2bob.json -w "%{http_code}" \
  "$BASE/v2/tenants/$TENANT/node?user_id=gu_bob&node_id=pr%3Aacme%2Fapp%2Fpr%2F7")
echo "http=$code body=$(cat /tmp/v2bob.json)"
test "$code" = "404"

echo "== metrics =="
curl -sf "$BASE/metrics" || true
echo

echo "SMOKE V2 OK"
