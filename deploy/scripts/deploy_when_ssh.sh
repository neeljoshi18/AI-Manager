#!/usr/bin/env bash
# One-shot: pull main, deploy staging, prune twins, reseed intent demo.
# Run from your Mac when agent SSH is blocked:
#   ./deploy/scripts/deploy_when_ssh.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
git pull origin main
./deploy/scripts/sync_and_deploy_staging.sh
echo "== post-deploy prune + intent seed =="
sleep 10
curl -sS --max-time 30 -X POST "https://status.neel.world/v3/tenants/ten_github/team/prune" || true
echo
curl -sS --max-time 30 -X POST "https://status.neel.world/v3/tenants/ten_github/seed/intent_demo" || true
echo
curl -sS --max-time 15 "https://status.neel.world/v3/tenants/ten_github/team" | python3 -c "
import sys,json
d=json.load(sys.stdin)
print('multi', d.get('multi_person_ready'), 'uniq_slack', d.get('unique_slack_users'), 'enabled', d.get('enabled_person_twins'))
for m in d.get('members') or []:
    if m.get('enabled') and m.get('twin_id'):
        print('-', m.get('display_name'), m.get('subject_id')[:36], m.get('slack_user_id'))
"
echo "Done. Hard-refresh https://status.neel.world/app/"
