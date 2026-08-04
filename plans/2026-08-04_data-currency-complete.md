# Data currency complete (2026-08-04)

Commit `7d8417a` closes the on-rails list from handoff 08-03b.

| Item | Implementation |
|------|----------------|
| Commit messages on graph titles | `map_push` title+message; snapshot `message`; Dev insights UI |
| Deeper poller / first-boot bulk | `COMMIT_BOOT_PAGES=15`, `COMMIT_BOOT_CAP=80`, force boot poll |
| Long-lived PAT | Prefer `GITHUB_PAT`/`BRIDGE_GITHUB_TOKEN`; boot token probe; Actions injects all three env keys |
| Dual digests | `seed/team_activity` + `seed/dual_digests` for empty neighborhoods; deploy smoke |
| Volumes sacred | Deploy lists volumes, never prune/`-V`, README doctrine |

## Smoke after green

```bash
curl -sf https://status.neel.world/v3/healthz
curl -sf https://status.neel.world/v3/tenants/ten_github/insights/dev | jq '.activity.insight, .recent_commits[0], .graph'
curl -sf -X POST https://status.neel.world/v3/tenants/ten_github/team/compile \
  -H 'content-type: application/json' -d '{"force_notify":false,"allow_notify":false}' \
  | jq '{with_items, results: [.results[]|{name:.display_name,items:.item_count,empty:.empty,reason:.empty_reason}]}'
```

Expect: recent_commits with real messages; with_items ≥ 2 after dual seed.
