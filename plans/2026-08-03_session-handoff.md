# Session Handoff — Context Transfer

**Date:** 2026-08-03  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager` (branch `main`)  
**Tip (at handoff write):** `9bd8f99` — confirm with `git log -5 --oneline`  
**Purpose:** Full handoff for a **new chat**. Prefer this file + listed ground-truth docs over compacted history.  
**Supersedes for “what to do next”:** `Session Handoff_ Context Transfer 2026-07-31.md` (keep for older arc).  

**Related plans:**  
- `plans/2026-08-03_workspace-cleanup-map.md` ← **cleanup in progress (folder-by-folder)**  
- `plans/2026-08-03_durability-and-fast-deploy.md`  
- `plans/2026-08-03_sales-call-readiness.md`  
- `plans/2026-08-01_no-deploy-done-log.md`  
- `plans/2026-07-31_batch-until-deploy.md`  
- `plans/2026-07-27_confidence-airtight-pilot.md`  

**Context budget:** If the session approaches **~399k / 500k context**, **stop work and re-handoff**. Do **not** trigger auto-compaction; write progress into this handoff / cleanup map instead.

---

## 0. Standing rules (forever)

| Rule | Detail |
|------|--------|
| **Airtight or don’t ship** | End-to-end, observable, non-spammy, stranger-readable |
| **Product** | Permissioned engineering context + meeting elimination. Anti-Glean. Not Buzz/Centaur |
| **Ingest continuous; Slack rare** | Notify Policy v1. Bridge never DMs |
| **Secrets** | Egress vault only. No silent 1:1 wiretap. No local training yet (ADR-016) |
| **UI** | Approve / Edit / Don’t send |
| **Staging** | https://status.neel.world/app/ — droplet on unless founder says off |
| **Campus Wi‑Fi** | TCP 22 often blocked. Prefer **git push → GitHub Actions**. Secrets already set |
| **Deploy speed** | Prefer `skip_build=true` or `deploy_fast.sh` service-scoped rebuilds; full cargo rebuild still slow on 2vCPU |
| **Durability** | Embedded disk journals on **Docker volumes** — never `docker volume prune` on staging or local data volumes you care about |
| **Cleanup** | Folder-by-folder; **one folder per agent turn** when founder is moving; update cleanup map after each |
| **Handoff hygiene** | Update this file + Interaction Log on key decisions |

---

## 1. Prompt to paste into the next session

```
You are continuing the AI Manager monorepo (private GitHub neeljoshi18/AI-Manager, branch main).

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-03.md  ← THIS FILE
2. plans/2026-08-03_workspace-cleanup-map.md  ← cleanup progress (folder-by-folder)
3. plans/2026-08-03_durability-and-fast-deploy.md
4. plans/2026-08-03_sales-call-readiness.md
5. plans/2026-08-01_no-deploy-done-log.md
6. starting-out-documents/Architecture Decision Log_ Pivotal Choices.md (ADR-011–016)
7. deploy/scripts/setup_ssh_via_https_port.md + deploy/scripts/deploy_fast.sh
8. Code as needed: twin-api, twin-compiler, graph-core membership/store, telemetry-core store/acl, docker-compose.app.yml

Ground rules:
- NEVER half-ass. Ship airtight end-to-end or don't ship.
- Product: permissioned engineering context + meeting elimination. Anti-Glean.
- Continuous ingest; Slack BATCHED + Notify Policy v1. Bridge never DMs.
- Secrets only via egress vault. No local model training yet (ADR-016).
- Approve / Edit / Don't send in product UI.
- Staging: https://status.neel.world/app/
- Campus: do NOT spam SSH. Prefer git push → GitHub Actions (STAGING_* secrets already set).
- Fast deploy: skip_build=true or deploy_fast.sh for one service; full rebuild only when Rust crates change.
- Durability: Docker volumes hold v1_acl / v1_events / v2_graph / v2_membership / twin_state — NEVER volume prune those.
- Cleanup: continue folder-by-folder from cleanup map; ONE folder per request if founder says so; report what was deleted.
- Save plans under plans/YYYY-MM-DD_slug.md. Commit and push when slices complete.
- ALWAYS update handoff + cleanup map + Interaction Log on key decisions.
- CONTEXT BUDGET: if approaching ~399k/500k tokens, STOP, update handoff, do NOT auto-compact.

Mission: (A) finish workspace cleanup folder-by-folder when asked; (B) keep pilot sales-demo ready; (C) dual digests when second human has GH activity.

Start by confirming you read the 2026-08-03 handoff (cleanup status, durability, campus deploy, sales bar), then either continue the next cleanup folder OR product work as the founder directs.
```

---

## 2. Files to attach / read first (new session)

### Always

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-03.md` ← **this file**  
2. `plans/2026-08-03_workspace-cleanup-map.md`  
3. `plans/2026-08-03_durability-and-fast-deploy.md`  
4. `plans/2026-08-03_sales-call-readiness.md`  
5. `plans/2026-08-01_no-deploy-done-log.md`  
6. `starting-out-documents/Architecture Decision Log_ Pivotal Choices.md`  
7. `starting-out-documents/Interaction Log_ Product Decisions.md`  
8. `starting-out-documents/Design Partner_ One-Pager.md`  
9. `starting-out-documents/Design Partner_ Install Runbook.md`  

### Deploy / campus

10. `deploy/scripts/setup_ssh_via_https_port.md`  
11. `deploy/scripts/deploy_fast.sh`  
12. `deploy/scripts/deploy_when_ssh.sh`  
13. `.github/workflows/deploy-staging.yml`  
14. `deploy/docker-compose.app.yml`  
15. `deploy/README.md`  

### Core product / durability code

16. `vertical-3/crates/twin-api/src/main.rs`  
17. `vertical-3/crates/twin-compiler/src/lib.rs`  
18. `vertical-3/crates/twin-delivery/src/worker.rs`  
19. `vertical-2/crates/graph-core/src/store.rs`  
20. `vertical-2/crates/graph-core/src/membership.rs`  
21. `vertical-2/crates/graph-api/src/main.rs`  
22. `vertical-1/crates/telemetry-core/src/store.rs`  
23. `vertical-1/crates/telemetry-core/src/acl.rs`  
24. `vertical-1/crates/telemetry-core/src/wiring.rs`  
25. `scripts/github_live_bridge.py`  
26. `vertical-3/app-static/index.html` + `app.js`  

### Optional prior

27. `starting-out-documents/Session Handoff_ Context Transfer 2026-07-31.md`  

---

## 3. Mental notes (do not lose)

### Ops

1. **Campus SSH:22 blocked** → Actions deploy. Secrets set 2026-08-03 via `gh secret set`: `STAGING_HOST`, `STAGING_USER`, `STAGING_SSH_KEY`.  
2. **Hotspot** enables interactive SSH + `deploy_fast.sh`.  
3. **sshd :2222** not configured (sudo password on droplet).  
4. **Full rebuild** still ~15–25 min cold; **skip_build** ~1–2 min; **one service** after cache warm is faster.  
5. **Docker Build Cache was ~21.7 GB** on founder Mac — pruned 2026-08-03. Do **not** `volume prune`.  
6. Agent sometimes **cannot read `~/Desktop/ai-manager`** (macOS TCC). Use `/tmp` clone or ask founder to run local `rm -rf */target` on Desktop.  
7. Staging: `206.189.129.31`, UI `https://status.neel.world/app/`.  

### Product / data

8. **Graph “gone” was misunderstanding**: V2 volume **did** reload (14 nodes / 17 edges). Membership was RAM-only (fixed). V1 events flush was weak (fixed to every write).  
9. Pre-journal history still unrecoverable.  
10. **paneerjeera** often `no_neighborhood` until real GH edges for that identity.  
11. Staging pilot window often **7d** (`STATUS_WINDOW_SECS=604800` on droplet `.env`) for sparse activity.  
12. **Demo seed** alice/bob hidden by default; `include_demo=true` / uncheck hide demo for seed UI.  
13. **pilot_readiness**: `GET /v3/tenants/ten_github/pilot_readiness` — soft outreach only when `soft_outreach_ready`.  
14. No Linear / training until dual digests proven (or founder explicitly asks).  

### Cleanup progress

15. **Done:** Docker build cache prune (~21.7 GB); `vertical-1/target` in work clone.  
16. **Next cleanup folder:** `vertical-2/` (`target/` ~228 MB in work clone).  
17. Then: `vertical-3/`, `vertical-security/`, `deploy/`, docs.  
18. Optional later: unused local platform images; Desktop monorepo `target/` if present.  

---

## 4. What shipped this arc (summary)

### Confidence / product

- A2 digests: 24h+ lookback, commits/pushes, multi-identity gu_*, empty→items fix  
- Server-side hide demo seed; pulse splits live vs demo conflicts  
- Team compile proof fields; pilot_readiness endpoint  
- Partner package aligned  

### Durability

- V1 events + ACL flush every write  
- V2 graph flush every write  
- V2 **membership** → `v2_membership.json`  
- Confirmed after full rebuild: graph file ~13.6 KB, membership ~517 B, nodes restored from disk  

### Deploy

- GitHub Actions campus path live  
- Sequential per-service builds (fix cargo race)  
- BuildKit cargo cache Dockerfiles  
- `deploy_fast.sh`  

### Cleanup (started)

- Mapped real disk hogs  
- Docker builder prune  
- vertical-1 target cleared (work clone)  
- Map: `plans/2026-08-03_workspace-cleanup-map.md`  

---

## 5. Order of execution (next session)

### If founder says “next folder” / continue cleanup

1. Read cleanup map.  
2. Clean **only `vertical-2/`** (or next pending): `target/`, logs, `.DS_Store`, `__pycache__` — **not** crates source.  
3. Report bytes freed + what kept.  
4. Update cleanup map. Commit if map/docs change.  
5. **Stop** and wait for “next folder” if founder is mobile.  

### If product / sales prep

1. HTTPS smoke: healthz, demo/status (durability block), team, pilot_readiness, graph.  
2. Demo path for sales (see sales-call plan).  
3. Dual digests only if second GH identity has edges.  

### If deploy

```bash
# Campus / no rebuild:
gh workflow run deploy-staging.yml -R neeljoshi18/AI-Manager -f skip_build=true

# Hotspot fast one service:
./deploy/scripts/deploy_fast.sh twin-api

# Full rebuild (slow):
gh workflow run deploy-staging.yml -R neeljoshi18/AI-Manager -f skip_build=false
```

---

## 6. A-list status (pilot)

| ID | Item | Status |
|----|------|--------|
| A1 | Notify non-spam | Done |
| A2 | Multi-person digests | Plumbing done; live often **1/2** (neel has items; paneerjeera needs GH) |
| A3 | Graph durability | **Code + live volume confirm** 2026-08-03 |
| A4 | Approve/Edit/Don’t send | Done |
| A5 | Install runbook | Done |
| A6 | Empty draft UX | Done + upgrade fix |
| A7 | Packaging | Done |

**Sales:** Demo-able now with honest 1-person digest + map + graph. Soft outreach dual-digest story needs 2nd human activity.

---

## 7. Commands

```bash
# Size map
docker system df
du -sh */target 2>/dev/null

# Cleanup (safe)
docker builder prune -af          # build cache only
rm -rf vertical-2/target          # next folder example

# Staging HTTPS
curl -sS https://status.neel.world/healthz
curl -sS https://status.neel.world/v3/demo/status
curl -sS https://status.neel.world/v3/tenants/ten_github/pilot_readiness
curl -sS https://status.neel.world/v3/tenants/ten_github/graph?node_limit=20

# Tests
cd vertical-1 && cargo test -p telemetry-core --lib
cd vertical-2 && cargo test -p graph-core --lib
cd vertical-3 && cargo test -p twin-compiler --lib && cargo test -p twin-core --lib
```

---

## 8. Document control

| Field | Value |
|-------|--------|
| Owner | Founder + coding agents |
| Update policy | On key decisions + after each cleanup folder |
| Prior handoff | `Session Handoff_ Context Transfer 2026-07-31.md` |
| Cleanup map | `plans/2026-08-03_workspace-cleanup-map.md` |
| Context stop | **~399k/500k — stop & re-handoff; no auto-compaction** |
