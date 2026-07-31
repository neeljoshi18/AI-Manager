#!/usr/bin/env bash
# Start local full stack for product demo (V1+V2+V3+egress+optional bridge).
# Usage: from monorepo root: ./scripts/dev_up.sh
# Stop: ./scripts/dev_down.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGDIR="${LOGDIR:-/tmp/ai-manager-dev}"
mkdir -p "$LOGDIR"
PIDFILE="$LOGDIR/pids"

kill_port() {
  local p="$1"
  if command -v lsof >/dev/null 2>&1; then
    local pids
    pids=$(lsof -ti ":$p" 2>/dev/null || true)
    if [[ -n "${pids:-}" ]]; then
      # shellcheck disable=SC2086
      kill $pids 2>/dev/null || true
      sleep 0.5
    fi
  fi
}

echo "== AI Manager dev_up =="
echo "logs: $LOGDIR"
: > "$PIDFILE"

# Free ports if stale
for p in 18080 18082 18083 18090; do
  kill_port "$p"
done

start_bg() {
  local name="$1"
  shift
  echo "starting $name..."
  (
    cd "$ROOT"
    "$@"
  ) >"$LOGDIR/$name.log" 2>&1 &
  echo $! >> "$PIDFILE"
  echo "  pid $! → $LOGDIR/$name.log"
}

# Egress (optional secrets)
if [[ -f "$ROOT/vertical-security/secrets/dev_secrets.json" ]]; then
  start_bg egress bash -c "cd vertical-security && cargo run -q -- --bind 0.0.0.0:18090 --registry config/tool_registry.yaml --secrets secrets/dev_secrets.json"
else
  echo "WARN: no vertical-security/secrets/dev_secrets.json — Slack real DMs disabled"
  echo "      copy secrets/dev_secrets.example.json and set SLACK_BOT_TOKEN"
fi

start_bg v1 bash -c "cd vertical-1 && SKIP_AUTH=true RUNTIME_MODE=embedded HTTP_BIND=0.0.0.0:18080 cargo run -q -p telemetry-ingestion"
start_bg v2 bash -c "cd vertical-2 && RUNTIME_MODE=embedded cargo run -q -p graph-api"

# Wait for V1/V2 briefly so V3 overlay works
for i in $(seq 1 40); do
  if curl -sf --max-time 1 http://127.0.0.1:18080/healthz >/dev/null 2>&1 \
    && curl -sf --max-time 1 http://127.0.0.1:18082/healthz >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

EGRESS_ARGS=()
if curl -sf --max-time 1 http://127.0.0.1:18090/healthz >/dev/null 2>&1; then
  EGRESS_ARGS=(USE_EGRESS_SLACK=true EGRESS_PROXY_URL=http://127.0.0.1:18090 EGRESS_ENFORCE=true)
fi

start_bg v3 bash -c "cd vertical-3 && \
  RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 \
  V2_BASE_URL=http://127.0.0.1:18082 \
  STATUS_WINDOW_SECS=86400 NOTIFY_INTERVAL_SECS=1800 COMPILE_INTERVAL_SECS=1800 \
  NOTIFY_ON_COMPILE=false \
  ${EGRESS_ARGS[*]:-} \
  cargo run -q -p twin-api"

if [[ "${START_BRIDGE:-1}" == "1" ]]; then
  start_bg bridge bash -c "TENANT_ID=ten_github python3 scripts/github_live_bridge.py"
fi

echo
echo "Waiting for V3..."
for i in $(seq 1 60); do
  if curl -sf --max-time 1 http://127.0.0.1:18083/healthz >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

echo
echo "════════════════════════════════════════"
echo "  Product UI:  http://127.0.0.1:18083/app/"
echo "  Lab console: http://127.0.0.1:18083/demo/"
echo "  V1 :18080  V2 :18082  V3 :18083  egress :18090"
echo "  Logs: $LOGDIR"
echo "  Stop: ./scripts/dev_down.sh"
echo "  ngrok (optional): ngrok http 18080"
echo "════════════════════════════════════════"
curl -sf http://127.0.0.1:18083/v3/demo/status 2>/dev/null | head -c 400 || true
echo
