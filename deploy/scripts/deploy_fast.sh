#!/usr/bin/env bash
# Fast staging deploy: rsync + rebuild only changed services (BuildKit cargo cache).
# Usage:
#   ./deploy/scripts/deploy_fast.sh              # auto-detect which services to rebuild
#   ./deploy/scripts/deploy_fast.sh twin-api     # rebuild only twin-api
#   ./deploy/scripts/deploy_fast.sh --no-build   # rsync + compose up (no image rebuild)
#   SERVICES=v2,twin-api ./deploy/scripts/deploy_fast.sh
set -euo pipefail
# PAUSED 2026-08-15 — DigitalOcean droplet is powered off. Script kept (not deleted).
# Local: ./scripts/dev_up.sh → http://127.0.0.1:18083/app/
# See: deploy/PAUSED_DIGITALOCEAN.md
echo "PAUSED: DigitalOcean droplet is off. Use ./scripts/dev_up.sh → http://127.0.0.1:18083/app/" >&2
exit 1
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOST="${STAGING_USER:-neel}@${STAGING_HOST:-206.189.129.31}"
DOMAIN="${DOMAIN:-status.neel.world}"
cd "$ROOT"

NO_BUILD=0
FORCE_SERVICES=""
for a in "$@"; do
  case "$a" in
    --no-build) NO_BUILD=1 ;;
    *) FORCE_SERVICES="${FORCE_SERVICES:+$FORCE_SERVICES,}$a" ;;
  esac
done

if ! ssh -o ConnectTimeout=8 -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$HOST" 'echo ok' >/dev/null 2>&1; then
  echo "SSH failed (campus?). Use: gh workflow run deploy-staging.yml -R neeljoshi18/AI-Manager -f skip_build=false"
  exit 2
fi

echo "== rsync (preserve droplet secrets + docker volumes) =="
rsync -az --delete \
  --exclude '**/target/' --exclude '.git/' --exclude 'ssh/' \
  --exclude '**/.DS_Store' --exclude '**/node_modules/' \
  --exclude 'vertical-security/secrets/dev_secrets.json' \
  --exclude 'deploy/.env.staging' \
  -e "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new" \
  "$ROOT/" "$HOST:~/ai-manager/"

# Decide services to rebuild from local git diff vs DEPLOYED_SHA on host
detect_services() {
  local remote_sha
  remote_sha=$(ssh -o BatchMode=yes "$HOST" 'cat ~/ai-manager/.deployed_sha 2>/dev/null || echo none')
  local local_sha
  local_sha=$(git rev-parse HEAD)
  echo "local=$local_sha remote=$remote_sha" >&2
  if [[ "$remote_sha" == "$local_sha" ]]; then
    echo ""
    return
  fi
  local base="$remote_sha"
  if ! git cat-file -e "${remote_sha}^{commit}" 2>/dev/null; then
    base="HEAD~1"
  fi
  local changed
  changed=$(git diff --name-only "$base" HEAD 2>/dev/null || git diff --name-only HEAD~1 HEAD)
  local svc=()
  echo "$changed" | grep -qE '^vertical-1/' && svc+=(v1)
  echo "$changed" | grep -qE '^vertical-2/' && svc+=(v2)
  echo "$changed" | grep -qE '^vertical-3/' && svc+=(twin-api)
  echo "$changed" | grep -qE '^vertical-security/' && svc+=(egress)
  echo "$changed" | grep -qE '^scripts/github_live_bridge.py' && svc+=(bridge)
  echo "$changed" | grep -qE '^deploy/docker-compose' && svc+=(v1 v2 twin-api egress bridge)
  # static-only twin change still needs twin-api rebuild (assets baked in image)
  printf '%s\n' "${svc[@]:-}" | sort -u | tr '\n' ' '
}

SERVICES="${SERVICES:-$FORCE_SERVICES}"
if [[ -z "$SERVICES" && "$NO_BUILD" -eq 0 ]]; then
  SERVICES=$(detect_services)
fi
SERVICES=$(echo "$SERVICES" | tr ',' ' ' | xargs)

echo "== remote compose (services to rebuild: ${SERVICES:-none}) =="
ssh -o BatchMode=yes -o ServerAliveInterval=30 -o StrictHostKeyChecking=accept-new "$HOST" \
  "DOMAIN=$DOMAIN NO_BUILD=$NO_BUILD SERVICES='$SERVICES' LOCAL_SHA=$(git rev-parse HEAD) bash -s" <<'REMOTE'
set -euo pipefail
cd "$HOME/ai-manager"
export DOMAIN COMPOSE_PARALLEL_LIMIT=1 DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1
test -f vertical-security/secrets/dev_secrets.json
test -f deploy/.env.staging

if [[ "${NO_BUILD}" == "1" || -z "${SERVICES// }" ]]; then
  echo "compose up (no image rebuild)"
  docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d
else
  echo "compose up --build $SERVICES"
  # shellcheck disable=SC2086
  docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d --build $SERVICES
  # Ensure dependents are running
  docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d
fi
echo "$LOCAL_SHA" > "$HOME/ai-manager/.deployed_sha"
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls ps
curl -sf --max-time 5 http://127.0.0.1:18083/healthz || true; echo
curl -sf --max-time 5 http://127.0.0.1:18082/healthz || true; echo
# Durability files on volumes
docker run --rm -v ai-manager_v1_state:/d alpine ls -la /d || true
docker run --rm -v ai-manager_v2_state:/d alpine ls -la /d || true
REMOTE

echo "== smoke =="
sleep 8
BASE="https://$DOMAIN"
curl -sS --max-time 20 "$BASE/healthz"; echo
curl -sS --max-time 20 "$BASE/v3/demo/status" | head -c 500; echo
curl -sS --max-time 20 -X POST "$BASE/v3/tenants/ten_github/graph/ensure_users" | head -c 300; echo
echo "Done. Fast path: only rebuilt [$SERVICES]. Volumes never deleted by deploy."
echo "UI: $BASE/app/"
