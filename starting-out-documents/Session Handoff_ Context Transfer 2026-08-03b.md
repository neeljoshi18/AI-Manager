# Session Handoff — Context Transfer (data is the product)

**Date:** 2026-08-03 (evening arc — **b**)  
**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Purpose:** New chat ground truth. **Data is currency. Never lose graph/commit history. Gas, not brakes.**  
**Supersedes “what next”:** `Session Handoff_ Context Transfer 2026-08-03.md` (keep for cleanup map).  
**Context discipline:** Prefer a **new session** with this handoff over auto-compact. Founder will open a fresh chat — **do not auto-compact** this arc; soft stop ~480–490k only if still mid-flight.

---

## 0. Founder energy / non-negotiables (read this first)

This is not optional theater. Founder is dogfooding the product **while building it**. Commits that build the product should **shape** the product. That is the whole point.

1. **Data is gold / currency.** Every GitHub commit/push/PR that hits the pilot tenant must land in V1 → bridge → V2 and **survive deploys** (Docker volumes). Do not lose it. Map it. Draw insights from it.
2. **CLI, Actions, GitHub UI, whatever — same path.** If it got into git, the platform monitors it. Webhooks **and** commit poller backfill so nothing is lost when V1 blips.
3. **Dev insights are the product surface for founder dogfood.** Activity heat (when most active), commit volume, authors, by-day bars, empty vs full digests — live under **Dev insights** in the app. Maximum data, tabulated, usable.
4. **Do not ask for hotspot** for routine ops. Campus Wi‑Fi blocks SSH:22; **tunnel/path is GitHub Actions** (`STAGING_*` secrets already set). `git push` HTTPS → Actions rsync/SSH from runner → droplet. Only founder-only secrets/legal stop you. Asking “get on hotspot” for deploy is **wrong rails**.
5. **Do not ask permission** for on-rails work. Vision + rails already set. Execute: restart V1, backfill commits, deploy, insights UI, durability, smoke proofs. Report results. Gas, not brakes.
6. **No circular bullshit.** Prefer end-to-end proof: push → graph node appears → Dev insights updates. Airtight or don’t ship.
7. **VPS fees are for this to work.** Staging must be real series, not half-alive V1 + empty digests theater.
8. **Suggestions that are on-rails = just do them.** Do not pause for approval loops on durable volumes, pollers, insights, autoheal, Actions deploy.

---

## 1. Paste prompt (next session)

```
You are continuing the AI Manager monorepo (private GitHub neeljoshi18/AI-Manager, branch main).

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-03b.md  ← THIS FILE (motivation + data doctrine)
2. plans/2026-08-03_data-currency-and-dev-insights.md
3. plans/2026-08-03_workspace-cleanup-map.md
4. plans/2026-08-03_durability-and-fast-deploy.md
5. plans/2026-08-03_sales-call-readiness.md
6. Code: scripts/github_live_bridge.py (commit poller), vertical-3 twin-api GET …/insights/dev, app-static Dev insights, deploy/docker-compose.app.yml, .github/workflows/deploy-staging.yml

Doctrine (non-negotiable):
- DATA IS CURRENCY. Never lose commits. Map CLI / GitHub UI / Actions the same. Poller + webhook.
- Do NOT ask for hotspot for routine ops — use GitHub Actions deploy (STAGING_HOST / USER / SSH_KEY set).
- Do NOT ask permission for on-rails work — execute, prove E2E, report.
- Notify Policy v1; bridge never DMs; secrets via egress vault; no training yet.
- Staging app: https://status.neel.world/app/  · Dev insights nav tab
- Airtight or don't ship. Anti-Glean. Meeting elimination. Founder is the test pig — dogfood is the product.

Mission:
1. 100% commit coverage (webhook + poller + seen file on volume)
2. Durable volumes never pruned on staging data
3. Dev insights live: peak hour/DoW, authors, by-day, recent commits, digest content ratio
4. Sales demo solid (map + digests + Approve + anti-spam)
5. Dual digests when 2nd human (paneerjeera) has GH edges

Start by confirming you read 2026-08-03b (data doctrine, no-hotspot, no-permission-brakes), smoke:
  curl -sf https://status.neel.world/v3/healthz
  curl -sf 'https://status.neel.world/v3/tenants/ten_github/insights/dev' | head -c 800
  curl -sf 'https://status.neel.world/v3/tenants/ten_github/graph?node_limit=20' | head -c 600
then ship the next airtight gap without waiting.
```

---

## 2. Attach list (new session)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-03b.md`  
2. `plans/2026-08-03_data-currency-and-dev-insights.md`  
3. `plans/2026-08-03_workspace-cleanup-map.md`  
4. `plans/2026-08-03_durability-and-fast-deploy.md`  
5. `plans/2026-08-03_sales-call-readiness.md`  
6. `scripts/github_live_bridge.py`  
7. `vertical-3/app-static/app.js` + `index.html`  
8. `vertical-3/crates/twin-api/src/main.rs`  
9. `deploy/docker-compose.app.yml`  
10. `.github/workflows/deploy-staging.yml`  

---

## 3. Standing technical rails

| Topic | Truth |
|-------|--------|
| Product | Permissioned context + meeting kill. Not Glean/Buzz/Centaur |
| Path | GitHub → V1 webhook **and** commit poller → bridge → V2 → V3 digests + **Dev insights** |
| Durability | Volumes: v1_acl, v1_events (flush every write), v2_graph, v2_membership, twin_state, bridge seen files |
| Deploy | Actions `Deploy staging`; `skip_build=true` restart-only; full build when Rust/bridge changes. Campus = push only |
| Staging | https://status.neel.world/app/ · host often `206.189.129.31` via secrets |
| Never | `docker volume prune` on staging data; spam SSH from campus; half-shipped theater; permission asks for on-rails |

---

## 4. Why commits were “missing” (lesson — do not repeat)

- Pushes to `main` **always** hit GitHub (CLI, UI, Actions merge).  
- Graph only updates when **V1 is up** and bridge projects.  
- V1 was often **down/hung** after recreate → webhooks drop → graph stuck on old SHAs.  
- Fix doctrine: **auto-restart V1 in deploy**, **autoheal label**, **poller backfill** (GitHub Commits API → synthetic push events → V2), **Dev insights** so founder sees the currency.  
- Private repo needs **`GITHUB_TOKEN` / `BRIDGE_GITHUB_TOKEN`** on droplet `.env.staging` for poller rate/auth.

---

## 5. What this arc shipped (LIVE on staging — `d9c98ec`)

| Piece | Role |
|-------|------|
| `scripts/github_live_bridge.py` | `poll_github_commits`, seen file, synthetic push, seed `gu_*` |
| `GET /v3/tenants/{id}/insights/dev` | Peak hour/DoW UTC, by_author, by_day, commit nodes, digest content ratio |
| app-static **Dev insights** | Nav + heat bars + authors + recent commits — https://status.neel.world/app/ |
| compose | `COMMIT_POLL_*` + `GITHUB_TOKEN` on **bridge**; V1 `autoheal: true` |
| workflow | Post-up restart V1 + bridge; inject `BRIDGE_GITHUB_TOKEN` → droplet `GITHUB_TOKEN` |
| Secret | `BRIDGE_GITHUB_TOKEN` set on repo (from `gh auth token` — rotate to long-lived PAT when convenient) |

### Post-deploy smoke (2026-08-03, after Actions green)

| Signal | Value |
|--------|--------|
| Doctrine | `data_is_currency` |
| Graph | ~28–29 nodes · 56 edges · **26 Commit** · Person + Repo |
| Edges | AUTHORED 27 · PUSHED_TO 27 · CHECKED 2 |
| Author | `neeljoshi18`: 27 |
| Peak | **05:00 UTC** (14) · **Mon** (32) |
| By day | 07-30:6 · 07-31:14 · 08-01:4 · **08-03:32** |
| Digests | 1/2 person twins with content |
| UI | `/app/` shows **Dev insights**; `refreshDevInsights` in app.js |

**Follow-ups still open:** store commit **message** on graph title (recent list shows sha7 only); deeper backfill pages; paneerjeera dual digest edges; long-lived PAT for poller.

---

## 6. Cleanup status (paused unless asked)

- Freed ~**22.5 GB** (Docker build cache + cargo targets v1/v2).  
- Source folders lean. Optional: Desktop `*/target`, unused local images.

---

## 7. Sales / pilot

- Demo: map + graph + digests + Approve path + anti-spam.  
- Dual soft-outreach: needs 2nd human GH edges.  
- **Dev insights** = founder dogfood surface — treat as product, not toy. Building prod commits **are** the training signal for the product story (not model training).

---

## 8. Ops cheatsheet (Actions, not hotspot)

```bash
# From any network that can reach GitHub HTTPS:
git push origin main
# or:
gh workflow run "Deploy staging" -R neeljoshi18/AI-Manager
gh workflow run "Deploy staging" -R neeljoshi18/AI-Manager -f skip_build=true

# Smoke after green:
curl -sf https://status.neel.world/v3/healthz
curl -sf 'https://status.neel.world/v3/tenants/ten_github/insights/dev' | jq '.activity.insight, .graph'
curl -sf 'https://status.neel.world/v3/tenants/ten_github/graph?node_limit=50' | jq '{nodes:(.nodes|length), edges:(.edges|length), by_type}'
```

Optional secret (once, any network that can set secrets):  
`BRIDGE_GITHUB_TOKEN` = classic/fine-grained PAT with `repo` read on `neeljoshi18/AI-Manager` → written into droplet `.env.staging` as `GITHUB_TOKEN` on deploy.

---

## 9. Document control

| Field | Value |
|-------|--------|
| Update | Every key data/ops decision |
| Prior | `…2026-08-03.md`, `…2026-07-31.md` |
| Compaction | **Do not auto-compact** if new session is starting — handoff + paste prompt instead |
| Founder line | “Data is the gold. Don’t lose it. Map it. Insights. Gas time.” |
