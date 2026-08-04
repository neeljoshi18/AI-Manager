#!/usr/bin/env bash
# Sales-call smoke — go/no-go before booking strangers.
# Usage: ./scripts/sales_smoke.sh [BASE_URL]
set -euo pipefail
BASE="${1:-https://status.neel.world}"
T=ten_github
fail=0
ok() { echo "  OK  $*"; }
bad() { echo "  FAIL $*"; fail=1; }

echo "== Sales smoke @ $BASE =="

code=$(curl -sS -o /tmp/ss_hz.json -w '%{http_code}' --max-time 20 "$BASE/healthz" || echo 000)
if [ "$code" = "200" ]; then ok "healthz"; else bad "healthz http=$code"; fi

code=$(curl -sS -o /tmp/ss_demo.json -w '%{http_code}' --max-time 25 "$BASE/v3/demo/status" || echo 000)
if [ "$code" = "200" ]; then
  node -e '
    const d=require("/tmp/ss_demo.json");
    const n=d.graph_nodes||0;
    console.log(JSON.stringify({v1:d.v1,v2:d.v2,v3:d.v3,graph_nodes:n,graph_status:d.graph_status}));
    // v2+v3+graph required for demo; v1 required for live webhook ingest (warn if down)
    if(d.v2&&d.v3&&n>0) process.exit(0);
    process.exit(1);
  ' && ok "demo/status v2+v3+graph" || bad "demo/status thin or down"
  node -e 'const d=require("/tmp/ss_demo.json"); process.exit(d.v1?0:2)' \
    && ok "v1 ingest up" || { echo "  WARN v1 down — live GitHub webhooks may drop until restart"; }
else
  bad "demo/status http=$code"
fi

code=$(curl -sS -o /tmp/ss_pr.json -w '%{http_code}' --max-time 25 "$BASE/v3/tenants/$T/pilot_readiness" || echo 000)
if [ "$code" = "200" ]; then
  node -e '
    const d=require("/tmp/ss_pr.json");
    const a2=d.checklist&&d.checklist.A2_multi_person_digests&&d.checklist.A2_multi_person_digests.ok;
    const multi=d.multi_person_ready;
    const content=d.content_people||0;
    console.log(JSON.stringify({soft:d.soft_outreach_ready,multi,content,a2,note:d.note}));
    if(multi && content>=1) process.exit(0);
    process.exit(1);
  ' && ok "pilot_readiness multi+content" || bad "pilot_readiness not sales-ready"
else
  bad "pilot_readiness http=$code"
fi

code=$(curl -sS -o /tmp/ss_g.json -w '%{http_code}' --max-time 25 \
  "$BASE/v3/tenants/$T/graph?node_limit=120&edge_limit=300&include_demo=false" || echo 000)
if [ "$code" = "200" ]; then
  node -e '
    const d=require("/tmp/ss_g.json");
    const n=(d.nodes||[]).length, e=(d.edges||[]).length;
    const by=d.by_type||{};
    console.log(JSON.stringify({n,e,by,edge_by:d.edge_by_type}));
    // Must show people + edges (not commit-only hairball trunc)
    if(n>0 && e>0 && (by.Person||0)>0) process.exit(0);
    process.exit(1);
  ' && ok "graph people+edges" || bad "graph missing people/edges (truncation bug?)"
else
  bad "graph http=$code"
fi

code=$(curl -sS -o /tmp/ss_team.json -w '%{http_code}' --max-time 25 "$BASE/v3/tenants/$T/team" || echo 000)
if [ "$code" = "200" ]; then
  node -e '
    const d=require("/tmp/ss_team.json");
    const m=d.members||[];
    const withD=m.filter(x=>x.last_digest&&x.last_digest.draft_id).length;
    console.log(JSON.stringify({members:m.length,with_draft:withD,multi:d.multi_person_ready}));
    if(withD>=1) process.exit(0);
    process.exit(1);
  ' && ok "team has openable draft" || bad "team missing drafts"
else
  bad "team http=$code"
fi

# App shell present
code=$(curl -sS -o /tmp/ss_app.html -w '%{http_code}' --max-time 15 "$BASE/app/" || echo 000)
grep -q 'My status\|Dev insights\|Enrich' /tmp/ss_app.html 2>/dev/null && ok "app shell" || bad "app shell missing chrome"

echo
if [ "$fail" -eq 0 ]; then
  echo "SALES SMOKE: GREEN — you can book test calls (honest dual/solo framing per pilot_readiness note)."
  exit 0
else
  echo "SALES SMOKE: RED — fix fails above before stranger calls."
  exit 1
fi
