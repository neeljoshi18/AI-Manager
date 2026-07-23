#!/usr/bin/env bash
# Sync monorepo + gitignored secrets to staging VPS and bring up TLS stack.
# Usage (from monorepo root, VPS must be ON):
#   ./deploy/scripts/sync_and_deploy_staging.sh
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

echo "== remote: swap (if needed) + compose up =="
ssh -o BatchMode=yes "$HOST" "DOMAIN=$DOMAIN REMOTE_DIR=$REMOTE_DIR bash -s" <<'REMOTE'
set -euo pipefail
cd "$HOME/$REMOTE_DIR"
# 4GB droplet: add swap once so Rust release builds do not OOM
if ! swapon --show | grep -q .; then
  if [[ ! -f /swapfile ]]; then
    sudo fallocate -l 4G /swapfile || sudo dd if=/dev/zero of=/swapfile bs=1M count=4096
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
  fi
  sudo swapon /swapfile || true
fi
test -f vertical-security/secrets/dev_secrets.json
test -f deploy/.env.staging
export DOMAIN
export COMPOSE_PARALLEL_LIMIT=1
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d --build
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls ps
echo "== health (local on host) =="
curl -sf http://127.0.0.1:18083/healthz || true
echo
curl -sf http://127.0.0.1:18080/healthz || true
echo
REMOTE

echo "== public HTTPS probe =="
sleep 8
curl -sI --max-time 45 "https://$DOMAIN/healthz" | head -20 || true
curl -sf --max-time 45 "https://$DOMAIN/v3/demo/status" | head -c 500 || true
echo
echo "Product UI: https://$DOMAIN/app/"
