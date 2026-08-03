# Plan: Data currency + Dev insights (2026-08-03)

## Doctrine

- **Data is currency.** Every commit that lands in git (CLI, GitHub UI, Actions, bots) must be mappable into the product graph and survive deploys.
- **Dogfood loop:** commits building the product shape digests + Dev insights — that *is* the product story.
- **Gas:** deploy via Actions on campus; no hotspot asks; no permission loops for on-rails work.

## Problem

1. Webhooks only fire while **V1 is healthy**. Staging V1 often hung after recreate → commits never projected.
2. Graph looked “stuck” on old SHAs even when GitHub had new history.
3. No founder-facing surface for **when am I most active**, author heat, commit inventory.

## Design

```
GitHub (source of truth)
  ├─ webhook → V1 events journal (volume)
  └─ REST commits API ← bridge poller (every ~90s)
         ↓
    bridge projects push-shaped events → V2 graph (volume)
         ↓
    twin-api GET /v3/tenants/{id}/insights/dev
         ↓
    app-static "Dev insights" view
```

### Commit poller (`scripts/github_live_bridge.py`)

- Env: `GITHUB_TOKEN` / `GH_TOKEN` / `GITHUB_PAT`, `GITHUB_REPOS`, `COMMIT_POLL_SECS`, `COMMIT_POLL_PAGES`, `COMMIT_SEEN_FILE` (on bridge volume).
- Fetches `/repos/{owner}/{repo}/commits` pages, skips seen SHAs.
- Seeds V1 user (`gu_*`) when possible; synthesizes push event with `attributes.source=commit_poller`.
- Caps per tick so V2 is not flooded; continues next interval.
- **Private repo requires token** on droplet `.env.staging`.

### Dev insights API

- `GET /v3/tenants/{tenant_id}/insights/dev`
- Snapshot as `bridge_reader` (node_limit 800 / edge_limit 2000).
- Returns: graph counts by type, commit_nodes, AUTHORED/PUSHED counts, `by_author`, `by_day`, hour histogram UTC, day-of-week, peak insight string, recent commits sample, person-twin digest content ratio.

### UI

- Nav: **Dev insights**
- Stats: commit nodes, authored edges, peak hour UTC
- ASCII heat for hours + by day; author list; recent commits on graph

### Ops

- Compose: pass poller env; V1 `autoheal: true`
- Workflow: after up, **restart v1 + bridge**; optional inject `BRIDGE_GITHUB_TOKEN` → `GITHUB_TOKEN` in `.env.staging`
- Never `docker volume prune` on staging data volumes

## Acceptance

| Check | Pass |
|-------|------|
| Push any commit to `main` | Within ~2–3 min (poller) or webhook latency, Commit node on graph |
| `insights/dev` | Non-zero heat when edges exist; peak hour string |
| UI | Dev insights loads without console errors |
| Deploy from campus | Actions green without local SSH |
| Restart stack | Graph node count does not collapse to zero (volumes) |

## Follow-ups (next sessions, still on-rails)

- Sort recent commits by time desc; show full message from graph attrs
- IST offset toggle on heat (founder local)
- Backfill beyond 5 pages on first boot
- Paneerjeera dual digests once 2nd human has GH authored edges
- Rate-limit / secondary rate handling for GitHub API
- PR / issue nodes in insights (not only commits)

## Non-goals this arc

- Linear integration
- Model training on tenant data
- Asking founder for hotspot for deploys
