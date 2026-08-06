# Session Handoff — Context Transfer 2026-08-06

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Droplet:** `206.189.129.31` (DigitalOcean)  
**Neon project:** AI-manager / branch `production` / DB `neondb`  
**Purpose:** Fresh session after Neon continuous dual-write + flywheel ingest work.  
**Do not auto-compact** — use this handoff + new chat when context is high.

---

## 0. Neon vs Docker — honesty (read first)

### What **is** on Neon today (live dual-write)

| Table | Source | Continuous? |
|-------|--------|-------------|
| `twin_events` | Approve / Don't send / sync / product actions | **Yes** |
| `twin_twins` | Person twins / team | **Yes** (on every persist) |
| `twin_slack_maps` | Chat maps | **Yes** |
| `twin_drafts` | Digests + status (published/vetoed/pending) | **Yes** |
| `twin_tenant_kv` | Roles, tomorrow focus, event_log blob | **Yes** |
| `twin_snapshot_json` | Full twin export blob | **Yes** on sync/persist |

**Secret:** GitHub Actions secret `OBSERVE_DATABASE_URL` → injected into `deploy/.env.staging` on deploy.  
**Status API:** `GET /v3/observe/status` → `external_db: true`, `continuous_mirror: true`.  
**Manual button:** Settings → “Force full re-sync → Neon” (optional; upserts, not required daily).

### What is **not** on Neon yet (still droplet Docker volumes)

| Volume / data | Path / role | Migrated? |
|---------------|-------------|-----------|
| **V1 event journal** | `ai-manager_v1_state` / webhooks + ingest ledger | **No** |
| **V2 graph** (nodes/edges/commits) | `ai-manager_v2_state` / live context map | **No** |
| **Bridge seen state** | `bridge_state` / poller cursors | **No** |
| Vault secrets | `vertical-security/secrets/dev_secrets.json` | **Never** (ADR-012; vault only) |

### Will “everything from Docker go to Neon” ever happen?

| Scope | Possible? | When |
|-------|-----------|------|
| Twin product state (team, digests, decisions, events) | **Done** | Now |
| V2 graph snapshot export (periodic JSON/table of nodes+edges) | **Yes, straightforward** | Next 1–2 sessions if sales needs SQL on commits/repos |
| V1 raw event journal → Postgres | **Yes, medium** | After graph export; or when production CRDB/Postgres is primary |
| Replace Docker volumes entirely (stateless containers) | **Yes, larger** | Multi-tenant packaging; twin-api production mode already has CRDB path |
| Secrets in Neon | **No** | Vault / DO secrets only |

**Bottom line:** You already have **continuous** Neon for the **status/digest/decision plane**. The **work graph flywheel** (commits/PRs) still lives on V2 volumes + is visible in the product Graph UI; putting **that** in Neon is a deliberate next slice, not free with the twin mirror.

---

## 1. Paste prompt (next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-06.md  ← THIS FILE
2. plans/2026-08-06_neon-you-do-this.md
3. plans/observe-neon.md
4. plans/2026-08-05_locality-inventory.md
5. plans/2026-08-04_operator-champion-ux-slate.md

Doctrine: Actions deploy not hotspot; gas; no Linear/training; vault for tokens; no LOC rankings.
Chat = delivery; GitHub = work. ADR-012 vault only. Do not break Slack default.

STAGING: https://status.neel.world/app/
NEON: OBSERVE_DATABASE_URL in GH secret; continuous dual-write for twin_* tables.
V1/V2 graph data still primarily on droplet volumes; Graph UI is live.

DONE:
- Champion cockpit, sales PDFs, bulk map, Connect Slack/GitHub install path
- DeliveryClient + Teams adapter (Slack default)
- Approve/Don't send + DM fallback if bot not in channel; Slack text approve
- Graph StatusDigest overlay + Show unapproved digests
- Multi-repo GitHub commit poller (GITHUB_REPOS_AUTO); flywheel UI strip
- Neon twin continuous mirror (twins/maps/drafts/kv/events/snapshot)
- Recover workflow force-recreates V1; /v1/healthz alias for Caddy

NEXT (priority order for sales flywheel + portable data):
1. Smoke: V1 stable, commits still climbing, external_db true
2. Optional: export V2 graph snapshot → Neon tables (nodes/edges) for SQL insights
3. Connect Slack/GH one real install path polish if needed on call
4. Sales narrative: efficiency metrics from heat/digests/conflicts (not LOC)
5. Roles soft gates; Teams only if prospect is Teams-first
6. Neon already continuous for twin plane — no manual sync required
7. Full volume-less multi-tenant later (CRDB/Postgres primary)

Campus SSH often blocked; use Actions recover_only for droplet ops.
Hotspot only if SSH/Web Console needed. Do not ask for secrets in chat.

Start: curl health + observe/status + pilot_readiness + insights commit counts.
Handoff again when context high — no auto-compact.
```

---

## 2. Attach list (next session)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-06.md` ← **this file**  
2. `plans/2026-08-06_neon-you-do-this.md`  
3. `plans/observe-neon.md`  
4. `plans/2026-08-05_locality-inventory.md`  
5. `plans/2026-08-04_operator-champion-ux-slate.md`  
6. `deploy/oauth/README.md`  
7. `deploy/docker-compose.app.yml`  
8. `vertical-3/crates/twin-api/src/observe.rs`  
9. `vertical-3/crates/twin-api/src/main.rs` (oauth, graph overlay, persist/mirror)  
10. `vertical-3/crates/twin-delivery/` (DeliveryClient, Teams, worker DM fallback)  
11. `vertical-3/app-static/app.js` + `index.html`  
12. `scripts/github_live_bridge.py` (multi-repo poller)  
13. `.github/workflows/deploy-staging.yml` (recover_only + secret inject)  
14. `Sales Call Documents/README.md` (optional on sales prep days)

---

## 3. Ops cheatsheet

```bash
# Stack
curl -s https://status.neel.world/v3/demo/status | jq '{v1,v2,v3,egress,graph_nodes,graph_status}'
curl -s https://status.neel.world/v3/observe/status | jq .
curl -s https://status.neel.world/v3/tenants/ten_github/pilot_readiness | jq '{soft_outreach_ready,content_people,multi_person_ready}'
curl -s https://status.neel.world/v3/tenants/ten_github/insights/dev | jq '{commits:.graph.commit_nodes,repos:.graph.by_type.Repo,authors:.activity.by_author}'

# Neon (optional force)
curl -s -X POST https://status.neel.world/v3/tenants/ten_github/sync_to_db | jq .

# Recover (Actions): Deploy staging → recover_only=true
# Secret: OBSERVE_DATABASE_URL (GitHub Actions secrets)
```

**Neon SQL:**

```sql
SELECT count(*) FROM twin_twins;
SELECT count(*) FROM twin_drafts;
SELECT kind, subject, at FROM twin_events ORDER BY at DESC LIMIT 20;
SELECT tenant_id, synced_at FROM twin_snapshot_json;
```

---

## 4. When can you **genuinely** sit on sales calls?

### Ready **now** (with honesty)

| Demo beat | Ready? | Notes |
|-----------|--------|-------|
| Staging URL works | **Yes** | status.neel.world/app |
| Champion cockpit | **Yes** | Pulse, digests, heat, flywheel strip |
| Multi-person digests | **Yes** | Soft outreach green when content on both |
| Graph of real work | **Yes** | Multi-repo commits (poller); not only AI-Manager |
| “Status that writes itself” | **Yes** | Compile + Notify Policy; Approve/Don't send |
| Connect Slack / GitHub buttons | **Mostly** | Real OAuth/install URLs; white-glove if creds/bot channel |
| Efficiency / heat insight | **Yes** | Dev insights heat/authors — **not** LOC rankings |
| Teams-only prospect | **Soft** | Adapter shipped; not default; Slack first |
| Self-serve multi-tenant | **No** | Single pilot tenant packaging |

### Sales stance (use this)

> “We white-glove connect your **GitHub** (work) and **Slack** (delivery). Digests land with **Approve / Edit / Don't send**. Champions get a cockpit: pod map, conflicts, heat — no standup theater, no productivity rankings. We’re on our own eng data flywheel first so the graph and digests are real.”

### Blockers to treat as **not** call-breaking

- Invite bot to team channel for channel posts (DM fallback works).  
- Neon is for **you** (ops/SQL), not a customer-facing requirement.  
- V1 can wedge on small VPS; recover force-recreates; commit poller still fills graph.  
- SSO / Google login not required for pilot.

### After 1–2 more slices → **stronger** calls

1. Graph → Neon export (SQL “show me every commit this week”)  
2. One recorded 5-minute demo path that never fails  
3. Optional “time not spent in standups” narrative numbers from digests + heat  

**Recommendation:** You can **take discovery + soft demo calls now**. Don’t promise self-serve Connect + multi-tenant SSO until packaging. Prefer **Slack + GitHub** prospects first.

---

## 5. Next-session mission (execute order)

1. Smoke V1/V2/egress/Neon continuous after any deploy.  
2. If sales wants SQL on graph: implement **V2 snapshot → Neon** tables (`graph_nodes`, `graph_edges`) on a timer.  
3. Harden V1 further if it flips red (watchdog already in recover).  
4. Connect Slack/GH call path: bot in channel + one install dry-run.  
5. Do **not** re-build cockpit/sales PDFs from scratch.  
6. Neon full volume replacement = later multi-tenant work, not blocking calls.

---

## 6. Explicit session boundary

| In **next** session | Do **not** |
|---------------------|------------|
| Graph→Neon export if needed | Restart sales PDF/cockpit from zero |
| V1 stability / flywheel | Linear / training / LOC rankings |
| Sales demo polish | Auto-compact this arc |
| | Put secrets in git or chat |

---

## 7. Document control

| Field | Value |
|-------|--------|
| Prior handoffs | `…2026-08-04c.md`, `…2026-08-05` plans |
| Neon how-to | `plans/2026-08-06_neon-you-do-this.md`, `plans/observe-neon.md` |
| Locality | `plans/2026-08-05_locality-inventory.md` |
| Slate | `plans/2026-08-04_operator-champion-ux-slate.md` |
| Compaction | **Handoff instead of auto-compact** |
