#!/usr/bin/env bash
# One-shot: pull main, deploy staging, prune twins, seed intents, ensure ACL users, compile digests.
# Run from your Mac on mobile hotspot when agent cannot SSH (campus Wi‑Fi blocks port 22):
#   ./deploy/scripts/deploy_when_ssh.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
echo "== git pull =="
git pull origin main
echo "== deploy (build on droplet — may take 15–25 min) =="
./deploy/scripts/sync_and_deploy_staging.sh
echo "== post-deploy product setup =="
sleep 12
BASE="https://status.neel.world"
curl -sS --max-time 20 "$BASE/healthz" || true
echo
curl -sS --max-time 30 -X POST "$BASE/v3/tenants/ten_github/team/prune" || true
echo
curl -sS --max-time 30 -X POST "$BASE/v3/tenants/ten_github/graph/ensure_users" || true
echo
curl -sS --max-time 30 -X POST "$BASE/v3/tenants/ten_github/seed/intent_demo" || true
echo
curl -sS --max-time 60 -X POST "$BASE/v3/tenants/ten_github/team/compile" \
  -H 'content-type: application/json' \
  -d '{"force_notify":false,"allow_notify":true}' || true
echo
echo "== team ==="
curl -sS --max-time 15 "$BASE/v3/tenants/ten_github/team" | python3 -c "
import sys,json
d=json.load(sys.stdin)
print('multi', d.get('multi_person_ready'), 'uniq_slack', d.get('unique_slack_users'), 'enabled', d.get('enabled_person_twins'))
for m in d.get('members') or []:
    if m.get('enabled') and m.get('twin_id'):
        ld = m.get('last_digest') or {}
        print('-', m.get('display_name'), m.get('subject_id')[:36], m.get('slack_user_id'),
              'digest', ld.get('status_label') or ld.get('status') or '—')
"
echo "== demo status ==="
curl -sS --max-time 15 "$BASE/v3/demo/status" | python3 -c "
import sys,json
d=json.load(sys.stdin)
print({k:d.get(k) for k in ['graph_status','graph_nodes','graph_edges','notify_policy','v1_accepted']})
"
echo
echo "Done. Hard-refresh https://status.neel.world/app/ (Cmd+Shift+R)"
echo "Expect: Team 2 people, Graph hide-demo, Today blockers, digests with commits if activity exists."
