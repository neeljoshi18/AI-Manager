#!/usr/bin/env bash
# Seed multi-person SHIP vs FREEZE + BLOCKED intents for Team blockers UI.
# Usage:
#   ./scripts/seed_intent_demo.sh
#   V2_BASE=https://status.neel.world ./scripts/seed_intent_demo.sh   # via caddy? needs /v2 proxy
# Prefer direct on host or twin-api proxy if present.
set -euo pipefail
V2_BASE="${V2_BASE:-http://127.0.0.1:18082}"
TENANT="${TENANT_ID:-ten_github}"
curl -sS --max-time 30 -X POST "$V2_BASE/v2/tenants/${TENANT}/seed/intent_demo" | python3 -m json.tool
echo
curl -sS --max-time 15 "$V2_BASE/v2/tenants/${TENANT}/conflicts?user_id=gu_demo_alice" | python3 -m json.tool | head -80
