#!/usr/bin/env bash
# Smoke test against running twin-api (default embedded on :18083)
set -euo pipefail
BASE="${BASE_URL:-http://127.0.0.1:18083}"
TENANT="${TENANT_ID:-ten_smoke}"

echo "== health =="
curl -sf "$BASE/healthz"
echo

echo "== upsert person twin (no shadow for smoke) =="
curl -sf -X POST "$BASE/v3/tenants/$TENANT/twins" \
  -H 'content-type: application/json' \
  -d '{
    "twin_kind": "person",
    "subject_id": "gu_alice",
    "display_name": "Alice",
    "channel_id": "C_SMOKE",
    "shadow_until": null,
    "high_auto_publish": false,
    "slack_user_id": "U_ALICE"
  }'
echo

# Clear default 10-day shadow: re-upsert with skip via compile skip_shadow
TWIN_ID="twin:person:gu_alice"

echo "== inject fixture graph (embedded) =="
curl -sf -X POST "$BASE/v3/tenants/$TENANT/fixtures" \
  -H 'content-type: application/json' \
  -d '{
    "global_user_id": "gu_alice",
    "view": {
      "nodes": [
        {"node_id":"person:gu_alice","node_type":"Person","display_name":"Alice","resource_id":"gu_alice","properties":{},"is_private":false},
        {"node_id":"pr:acme/app/pr/7","node_type":"PullRequest","display_name":"Smoke","resource_id":"acme/app/pr/7","properties":{"title":"Smoke PR"},"is_private":false}
      ],
      "edges": [
        {"edge_id":"authored:smoke","edge_type":"AUTHORED","from_node_id":"person:gu_alice","to_node_id":"pr:acme/app/pr/7","event_id":"smoke_evt_1","properties":{},"is_private":false}
      ],
      "states": [
        {"node_id":"pr:acme/app/pr/7","state_key":"lifecycle","state_value":"OPEN","event_id":"smoke_evt_1","as_of":"2026-07-22T00:00:00Z"}
      ],
      "blockers": []
    }
  }' || echo "(fixtures endpoint skipped — production mode?)"
echo

echo "== compile (skip_shadow) =="
COMP=$(curl -sf -X POST "$BASE/v3/tenants/$TENANT/twins/$TWIN_ID/compile" \
  -H 'content-type: application/json' \
  -d '{"skip_shadow": true}')
echo "$COMP" | head -c 500
echo

LEDGER_ID=$(echo "$COMP" | sed -n 's/.*"ledger_id":"\([^"]*\)".*/\1/p' | head -1)
DRAFT_ID=$(echo "$COMP" | sed -n 's/.*"draft_id":"\([^"]*\)".*/\1/p' | head -1)
echo "ledger_id=$LEDGER_ID draft_id=$DRAFT_ID"

if [[ -n "$LEDGER_ID" ]]; then
  echo "== get ledger =="
  curl -sf "$BASE/v3/tenants/$TENANT/ledgers/$LEDGER_ID"
  echo
fi

if [[ -n "$DRAFT_ID" ]]; then
  echo "== get draft =="
  curl -sf "$BASE/v3/tenants/$TENANT/drafts/$DRAFT_ID"
  echo
fi

echo "== metrics =="
curl -sf "$BASE/metrics" || true
echo

echo "SMOKE V3 OK"
