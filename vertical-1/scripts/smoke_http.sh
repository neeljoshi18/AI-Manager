#!/usr/bin/env bash
# End-to-end HTTP smoke against a running telemetry-ingestion process.
# Prerequisites:
#   SKIP_AUTH=true RUNTIME_MODE=embedded cargo run -p telemetry-ingestion
set -euo pipefail

BASE="${BASE_URL:-http://127.0.0.1:18080}"
TENANT="${TENANT_ID:-ten_demo}"
SECRET="${WEBHOOK_SECRET:-whsec_demo}"

echo "== health =="
curl -sf "$BASE/healthz" | tee /tmp/v1-health.json
echo

echo "== upsert tenant =="
curl -sf -X POST "$BASE/v1/tenants" \
  -H 'content-type: application/json' \
  -d "{\"tenant_id\":\"$TENANT\",\"github_webhook_secret\":\"$SECRET\",\"default_group_ids\":[\"grp_eng_core\"]}"
echo

echo "== seed user =="
USER_JSON=$(curl -sf -X POST "$BASE/v1/tenants/$TENANT/users" \
  -H 'content-type: application/json' \
  -d '{"provider_user_id":"42","email":"alice@acme.io","display_name":"Alice","groups":["grp_eng_core"]}')
echo "$USER_JSON"
USER_ID=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["global_user_id"])' <<<"$USER_JSON")

BODY='{"action":"opened","pull_request":{"number":1,"title":"Demo","state":"open","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","user":{"id":42,"login":"alice"},"base":{"ref":"main"},"head":{"ref":"feat"}},"repository":{"full_name":"acme/app","private":true},"sender":{"id":42,"login":"alice"}}'

echo "== ingest signed github webhook =="
if command -v openssl >/dev/null; then
  SIG="sha256=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $2}')"
else
  SIG=""
fi

curl -sf -X POST "$BASE/v1/tenants/$TENANT/webhooks/github" \
  -H 'content-type: application/json' \
  -H "X-GitHub-Event: pull_request" \
  -H "X-GitHub-Delivery: smoke-$(date +%s)" \
  ${SIG:+-H "X-Hub-Signature-256: $SIG"} \
  -d "$BODY"
echo

echo "== query events as allowed user =="
curl -sf "$BASE/v1/tenants/$TENANT/events?user_id=$USER_ID&limit=10"
echo

echo "== revoke eng group =="
curl -sf -X DELETE "$BASE/v1/tenants/$TENANT/users/$USER_ID/groups/grp_eng_core"
echo

echo "== query after revoke (expect count=0 for private PR) =="
curl -sf "$BASE/v1/tenants/$TENANT/events?user_id=$USER_ID&limit=10"
echo

echo "== metrics =="
curl -sf "$BASE/metrics"
echo
echo "SMOKE OK"
