# A2 digest lookback + dual-person proof surface (2026-07-31)

## Problem

Staging showed **empty digests** for real git activity because:

1. Live staging was **behind main** (no commit/push ledger path, no multi-identity merge).
2. Default **STATUS_WINDOW_SECS=3600** made pilot sparse-activity days look empty even after deploy.
3. Graph edges carried `valid_from` in V2 but twin-compiler **dropped** timestamps — no real lookback filter.
4. Team compile response lacked per-person **item summaries / empty_reason** for A2 proof.

## Shipped (this batch)

| Change | Why |
|--------|-----|
| Default **STATUS_WINDOW_SECS=86400** (compose, dev_up, config default) | 24h rolling lookback = standup replacement for sparse pilots |
| `activity_lookback(now)` + `CompileOpts.activity_*` | Rolling filter separate from aligned ledger_id bucket |
| `GraphEdgeView.valid_from` + http_v2 parse | Timestamped activity for lookback |
| Open PR/issue **always in** digest if linked | Ongoing work not dropped because open PR is “old” |
| Commits/pushes outside lookback **dropped** | Avoid infinite re-surface of ancient commits |
| Sort by recency; evidence `at:ISO` | Stranger-readable proof |
| Team compile: `with_items`, `item_kinds`, `item_summaries`, `empty_reason`, aliases ACL seed | A2 multi-person proof surface |
| Unit tests: multi-alias, window filter, dual-person distinct digests | Airtight without deploy |

## Still needs deploy for live A2 green

1. Founder: hotspot or Actions secrets → one deploy.
2. `ensure_users` → prune → seed intent → `team/compile`.
3. Expect **neeljoshi18** non-empty if graph has commits/pushes in last 24h (or open PRs).
4. **paneerjeera** stays empty until that GitHub identity has graph edges (real pushes/PRs) — correct, not a bug.

## Soft-outreach readiness (A1–A7)

| ID | Bar | Status |
|----|-----|--------|
| A1 | Anti-spam Notify v1 | Done live |
| A2 | Multi-person digests with real content | Plumbing + lookback done; **live dual non-empty** after deploy + 2nd human GH activity |
| A3 | Graph durability | Code done; verify after deploy |
| A4 | Approve / Edit / Don’t send | Done |
| A5 | Install runbook | Done |
| A6 | Empty draft UX | Done + empty_reason on compile |
| A7 | Packaging | Done; re-check runbook vs 24h window after deploy |

**Do not start Linear/training until A2 live dual digests proven.**
