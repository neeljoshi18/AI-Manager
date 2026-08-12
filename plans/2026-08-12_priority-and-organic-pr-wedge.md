# Priority order + organic PR wedge (2026-08-12)

## What was left in the dust

Cross-read of handoffs (08-03 → 08-08), adequacy experiment, next-upgrades, pilot backlog.

| Pri | Item | Status after this ship |
|-----|------|------------------------|
| **P0** | Project **real PRs** (not only commits) | **Shipped:** `poll_github_pulls` in bridge |
| **P0** | Organic intent from title/labels/body | **Unblocked:** existing `rules_v0` attach + labels on V1 webhook normalize |
| **P0** | Strip **seed BLOCKS** from digest `open_blockers` | **Shipped:** twin-compiler filter |
| **P0** | Honest live empty / demo split on pulse | **Shipped:** `include_demo=true` then split; `empty_reason` |
| P1 | Slack Events live + bot in channel | Code exists; needs human install |
| P1 | Morning digest env / Linear keys | Optional env — human |
| P1 | Role enforcement on writes | Not this wedge |
| P2 | CI projection, commitment Linear webhook, SSO | Later |

**Thesis blocker (adequacy):** trajectory strong, claims starved because **1 PR vs ~248 commits**. That was the neglected P0 while Simple/Technical + commitments shipped.

## What this change does

1. **Bridge PR poller** — same multi-repo flywheel as commits; projects `pull_request.*` with title, labels, body_preview, draft, merged → V2 PullRequest + Intent (`is_demo=false`, `source=github_pr`).
2. **V1 `normalize_pr` / issue** — pass labels + body_preview so webhooks classify like the poller.
3. **Digest hygiene** — never surface story-1 / graph_story / intent_demo BLOCKS as real “you are blocked”.
4. **Pulse honesty** — fetch conflicts/intents with `include_demo=true`, split live vs demo; empty states explain `no_friction` vs `only_demo_seeds`.

## Verify after deploy

```bash
curl -s https://status.neel.world/v3/tenants/ten_github/insights/dev | jq '.graph.by_type'
# expect PullRequest > 1, Intent growing with non-seed

curl -s https://status.neel.world/v3/tenants/ten_github/pulse | jq '.conflicts|{count,demo_count,empty_reason}'

curl -s https://status.neel.world/v3/tenants/ten_github/intent/engine | jq '.ledger_live,.ledger_all'

# Wait ~2–5 min after bridge restart for PR boot poll
python3 scripts/intent_adequacy_pack.py --base https://status.neel.world --tenant ten_github --subject neeljoshi18 --out plans/packs/
```

## Still needs human (not blocked for engineering)

- Invite Slack bot to team channel for ambient claim extract
- `COMMITMENT_DIGEST_CHANNEL` / Linear keys if morning digest / export desired
- Long-lived `GITHUB_PAT` on droplet if token is short-lived Actions oauth
