#!/usr/bin/env bash
# Smoke-check the credential egress proxy if it is already running.
# Does NOT start the proxy — run vertical-security first:
#   cd vertical-security && cp secrets/dev_secrets.example.json secrets/dev_secrets.json
#   cargo run
#
# Usage:
#   ./scripts/egress_smoke.sh
#   EGRESS_PROXY_URL=http://127.0.0.1:18090 ./scripts/egress_smoke.sh
set -euo pipefail

PROXY="${EGRESS_PROXY_URL:-http://127.0.0.1:18090}"
PROXY="${PROXY%/}"

echo "== egress health ($PROXY/healthz) =="
if ! curl -sf --max-time 2 "$PROXY/healthz"; then
  echo
  echo "FAIL: egress proxy not reachable at $PROXY"
  echo "Start it with:"
  echo "  cd vertical-security && cargo run"
  echo "Or: cd vertical-security && docker compose up --build -d"
  exit 1
fi
echo
echo

echo "== TC-S03 style: unknown host must be 403 =="
CODE=$(curl -s -o /tmp/egress-deny.body -w "%{http_code}" --max-time 5 \
  "$PROXY/proxy/evil.example.com/x" \
  -H "X-AI-Manager-Tool: github_api" || true)
echo "status=$CODE body=$(cat /tmp/egress-deny.body 2>/dev/null || true)"
if [[ "$CODE" != "403" ]]; then
  echo "FAIL: expected 403 for non-allowlisted host, got $CODE"
  exit 1
fi
echo "OK deny"

echo
echo "== allowlisted path form (may 401/200 from real GitHub depending on token) =="
CODE=$(curl -s -o /tmp/egress-gh.body -w "%{http_code}" --max-time 15 \
  "$PROXY/proxy/api.github.com/user" \
  -H "X-AI-Manager-Tool: github_api" || true)
echo "status=$CODE"
# Proxy must not 403 for allowlisted host (upstream 401/200/502 are ok for smoke)
if [[ "$CODE" == "403" ]]; then
  echo "FAIL: allowlisted host returned 403 (registry/secrets misconfigured?)"
  head -c 200 /tmp/egress-gh.body 2>/dev/null || true
  echo
  exit 1
fi
if [[ "$CODE" == "000" ]]; then
  echo "FAIL: no response"
  exit 1
fi
echo "OK route (upstream status $CODE — inject attempted)"

echo
echo "EGRESS SMOKE OK"
