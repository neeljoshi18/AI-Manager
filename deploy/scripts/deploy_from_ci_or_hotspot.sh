#!/usr/bin/env bash
# Deploy staging without needing local secrets if droplet already has them.
# Hotspot: SSH:22 works. Campus: use `gh workflow run deploy-staging.yml` instead.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${STAGING_USER:-neel}@${STAGING_HOST:-206.189.129.31}"
DOMAIN="${DOMAIN:-status.neel.world}"
cd "$ROOT"

if ! ssh -o ConnectTimeout=8 -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$HOST" 'echo ok' >/dev/null 2>&1; then
  echo "SSH to $HOST failed (campus Wi-Fi often blocks :22)."
  echo "Use: gh workflow run deploy-staging.yml -R neeljoshi18/AI-Manager"
  exit 2
fi

echo "== rsync code (preserve remote secrets) =="
rsync -az --delete \
  --exclude '**/target/' --exclude '.git/' --exclude 'ssh/' \
  --exclude '**/.DS_Store' --exclude '**/node_modules/' \
  --exclude 'vertical-security/secrets/dev_secrets.json' \
  --exclude 'deploy/.env.staging' \
  -e "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new" \
  "$ROOT/" "$HOST:~/ai-manager/"

echo "== remote compose =="
ssh -o BatchMode=yes -o ServerAliveInterval=30 -o StrictHostKeyChecking=accept-new "$HOST" bash -s <<REMOTE
set -euo pipefail
cd "\$HOME/ai-manager"
test -f vertical-security/secrets/dev_secrets.json
test -f deploy/.env.staging
export DOMAIN=$DOMAIN COMPOSE_PARALLEL_LIMIT=1
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d --build
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls ps
curl -sf --max-time 8 http://127.0.0.1:18083/healthz || true
echo
REMOTE

echo "== post-deploy smoke =="
sleep 12
BASE="https://$DOMAIN"
curl -sS --max-time 30 "$BASE/healthz"; echo
for path in team/prune graph/ensure_users seed/intent_demo; do
  echo -n "POST $path → "
  curl -sS --max-time 45 -o /dev/null -w "%{http_code}\n" -X POST "$BASE/v3/tenants/ten_github/$path" || echo fail
done
curl -sS --max-time 90 -X POST "$BASE/v3/tenants/ten_github/team/compile" \
  -H 'content-type: application/json' -d '{"force_notify":false,"allow_notify":true}' | head -c 800; echo
curl -sS --max-time 30 "$BASE/v3/tenants/ten_github/pilot_readiness" | head -c 1200; echo
echo "UI: $BASE/app/"
