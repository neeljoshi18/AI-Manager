#!/usr/bin/env bash
# Sync monorepo + gitignored secrets to staging VPS and bring up TLS stack.
# Usage (from monorepo root, VPS must be ON):
#   ./deploy/scripts/sync_and_deploy_staging.sh
#
# Swap setup is best-effort only (never blocks deploy). Non-interactive SSH
# cannot prompt for sudo passwords.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${STAGING_HOST:-neel@206.189.129.31}"
REMOTE_DIR="${STAGING_REMOTE_DIR:-ai-manager}"
DOMAIN="${DOMAIN:-status.neel.world}"

cd "$ROOT"
if [[ ! -f vertical-security/secrets/dev_secrets.json ]]; then
  echo "missing vertical-security/secrets/dev_secrets.json" >&2
  exit 1
fi
if [[ ! -f deploy/.env.staging ]]; then
  echo "missing deploy/.env.staging" >&2
  exit 1
fi

echo "== rsync → $HOST:~/$REMOTE_DIR =="
rsync -az --delete \
  --exclude '**/target/' \
  --exclude '.git/' \
  --exclude 'ssh/' \
  --exclude '**/.DS_Store' \
  --exclude '**/node_modules/' \
  -e "ssh -o BatchMode=yes" \
  "$ROOT/" "$HOST:~/$REMOTE_DIR/"

echo "== remote: compose up (swap is optional) =="
ssh -o BatchMode=yes "$HOST" "DOMAIN=$DOMAIN REMOTE_DIR=$REMOTE_DIR bash -s" <<'REMOTE'
set -euo pipefail
cd "$HOME/$REMOTE_DIR"

# Optional swap for small VPS builds — never fail deploy if sudo needs a password.
if command -v swapon >/dev/null 2>&1; then
  if ! swapon --show 2>/dev/null | grep -q .; then
    if [[ ! -f /swapfile ]]; then
      if sudo -n true 2>/dev/null; then
        echo "creating /swapfile (passwordless sudo)…"
        sudo -n fallocate -l 4G /swapfile 2>/dev/null \
          || sudo -n dd if=/dev/zero of=/swapfile bs=1M count=4096
        sudo -n chmod 600 /swapfile
        sudo -n mkswap /swapfile
        sudo -n swapon /swapfile || true
      else
        echo "skip swap: sudo needs a password (non-interactive). compose continues."
      fi
    else
      if sudo -n true 2>/dev/null; then
        sudo -n swapon /swapfile 2>/dev/null || true
      fi
    fi
  fi
fi

test -f vertical-security/secrets/dev_secrets.json
test -f deploy/.env.staging
export DOMAIN
export COMPOSE_PARALLEL_LIMIT=1
echo "== docker compose up -d --build =="
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d --build
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls ps
echo "== health (local on host) =="
curl -sf --max-time 5 http://127.0.0.1:18083/healthz || true
echo
curl -sf --max-time 5 http://127.0.0.1:18080/healthz || true
echo
REMOTE

echo "== public HTTPS probe =="
sleep 8
curl -sS --max-time 45 -o /dev/null -w "healthz http=%{http_code} time=%{time_total}\n" "https://$DOMAIN/healthz" || true
curl -sS --max-time 45 "https://$DOMAIN/healthz" || true
echo
curl -sS --max-time 45 "https://$DOMAIN/v3/demo/status" | head -c 500 || true
echo
echo "Product UI: https://$DOMAIN/app/"
