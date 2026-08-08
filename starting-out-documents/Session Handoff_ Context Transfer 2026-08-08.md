# Session Handoff — Context Transfer 2026-08-08

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Purpose:** Fresh session after Intent Engine + Commitments + **Simple/Technical presentation overhaul**.  
**Do not auto-compact** — handoff + new chat when context is high.

---

## 0. What this arc shipped (high level)

| Area | Status |
|------|--------|
| Neon continuous twin + graph export | Live |
| Connect Slack/GH polish | Live |
| Intent research + in-house engine | Live |
| Plain-English insights | Live |
| Commitments (I'll… / done) | Live |
| Person directory + digest + Linear export | Live |
| **Simple vs Technical view** | **Must verify after this handoff** — deep visual Simple board on Home; Technical = ops stack |

### Doctrine (unchanged)

- Actions deploy not hotspot; gas; no Linear/**training** as product core; vault for tokens; **no LOC rankings**  
- Chat = delivery; GitHub = work  
- ADR-012 vault only  
- Do **not** break Slack default  

---

## 1. Paste prompt (next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-08.md  ← THIS FILE
2. plans/intent-research.md
3. plans/2026-08-07_intent-engine-design.md
4. plans/2026-08-07_how-we-classify-intent-simple.md
5. plans/2026-08-08_next-upgrades-shipped.md
6. plans/2026-08-06_intent-adequacy-experiment.md

Doctrine: Actions deploy not hotspot; gas; no Linear-as-core/training; vault for tokens; no LOC rankings.
Chat = delivery; GitHub = work. ADR-012 vault only. Do not break Slack default.

STAGING: https://status.neel.world/app/
Hard-refresh (Cmd+Shift+R) so app.js?v=20260808c + styles load.

UX CONTRACT (critical — user already corrected twice):
- Simple vs Technical is PRESENTATION ONLY.
- Every feature exists in both modes.
- Simple = visual, plain English, non-intimidating (org story, metric tiles, cards).
- Technical = denser operator console (tags, IDs, JSON-ish stats).
- NOT feature-gating / hiding Graph/Lab/etc.
- Home simple panel: #ck-visual-home ; technical: #ck-ops-stack (CSS body.mode-simple .tech-panel hide / body.mode-technical .simple-panel hide).

Intent stack:
- GET …/intent/engine, …/intent/ledger, …/intent/insights
- POST …/intent/claims, supersede
- Commitments: GET/POST …/commitments, done/dismiss, digest, export_linear
- People directory: GET …/people/directory
- Profile: GET …/people/{subject}/profile

NEXT (verify first, then polish):
1. Hard-refresh staging; toggle Simple/Technical on Home, People, Work map, Connect, Rhythm, Settings — confirm layouts flip hard.
2. If Simple still feels technical: push further visual (fewer words, more tiles/charts, strip remaining jargon on Team/Status/Graph chrome).
3. Smoke: commit create, insights, digest preview, profile, observe/status.
4. Optional: morning digest channel env; Linear keys only if needed.
5. V1 may flip red after deploy — recover_only Actions.

Campus SSH often blocked — Actions recover_only. No secrets in chat.
Handoff when context high — no auto-compact.
Start: hard-refresh app UX check + curl insights/commitments/observe.
```

---

## 2. Attach list (next session)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-08.md` ← **this file**  
2. `plans/intent-research.md`  
3. `plans/2026-08-07_intent-engine-design.md`  
4. `plans/2026-08-07_how-we-classify-intent-simple.md`  
5. `plans/2026-08-08_next-upgrades-shipped.md`  
6. `plans/2026-08-06_intent-adequacy-experiment.md`  
7. `vertical-3/app-static/app.js`  
8. `vertical-3/app-static/index.html`  
9. `vertical-3/app-static/styles.css`  
10. `vertical-3/crates/twin-api/src/intent_engine.rs`  
11. `vertical-3/crates/twin-api/src/commitments.rs`  
12. `vertical-3/crates/twin-api/src/main.rs`  
13. `vertical-3/crates/twin-api/src/observe.rs`  
14. `plans/2026-08-06_neon-you-do-this.md` (if Neon ops)  

---

## 3. Simple / Technical — how it is supposed to work

| Layer | Simple | Technical |
|-------|--------|-----------|
| **CSS** | `.simple-panel` visible; `.tech-panel` hidden | reverse |
| **Home** | `#ck-visual-home` — headline story, 4 big metrics, Needs you / Promises / People / Rhythm | `#ck-ops-stack` — classic cockpit, flywheel pills, heat, graph JSON, ledgers |
| **Nav labels** | Home, My update, People, Work map, Connect, Rhythm, Advanced | Cockpit, My status, Team, Graph, Connections, Dev insights, Lab |
| **Toggle** | Top bar + nav foot + Settings; `localStorage ai_manager_ux_mode` | same |
| **Data** | Same APIs | Same APIs |

**Files:** `app.js` (`setUxMode`, `fillVisualHome`, `plainIntentType`, dual renders), `index.html` (`ck-visual-home` / `ck-ops-stack`), `styles.css` (`.viz-*`, panel hide rules).

**Cache bust:** `?v=20260808c` on CSS + app.js — **hard refresh required**.

---

## 4. Ops cheatsheet

```bash
# Stack
curl -s https://status.neel.world/v3/demo/status | jq '{v1,v2,v3,egress,graph_nodes}'
curl -s https://status.neel.world/v3/observe/status | jq '{external_db,graph_mirror,tables}'

# Intent + commitments
curl -s https://status.neel.world/v3/tenants/ten_github/intent/insights | jq '{headline,act_on_today}'
curl -s https://status.neel.world/v3/tenants/ten_github/commitments?status=open | jq '{open_count,commitments}'
curl -s https://status.neel.world/v3/tenants/ten_github/commitments/digest | jq '{open_count,text}'
curl -s https://status.neel.world/v3/tenants/ten_github/people/directory | jq '{count}'
curl -s https://status.neel.world/v3/tenants/ten_github/people/neeljoshi18/profile | jq '{subject,authored:.work_surface.authored_commit_count,digests:.digests.count}'

# Recover: Actions → Deploy staging → recover_only=true
```

**Env optional:** `COMMITMENT_DIGEST_CHANNEL`, `COMMITMENT_DIGEST_HOUR_UTC`, `LINEAR_API_KEY`, `LINEAR_TEAM_ID`, `OBSERVE_DATABASE_URL` (GH secret).

---

## 5. Product truth (intent / commitments)

- **Trajectory** (commits, graph) is strong; **organic typed intents** still sparse (many demo seeds tagged).  
- **Commitments** are the user-facing accountability loop (Commit/Minimi-inspired), not Jira.  
- **Insights API** turns tags → plain English for “What needs attention.”  
- **Never** LOC rankings; **never** private 1:1 wiretap as default.

---

## 6. Explicit session boundary

| In next session | Do not |
|-----------------|--------|
| Verify/fix Simple visual board across all tabs | Restart sales PDFs/cockpit from zero |
| Further de-jargon Team/Status/Graph if still harsh | Auto-compact this arc |
| Flywheel / V1 stability | Put secrets in chat |
| Optional digest/Linear env wiring | Pretend demo intents are live org intelligence |

---

## 7. Document control

| Field | Value |
|-------|--------|
| Prior handoff | `…2026-08-06.md` |
| Intent research | `plans/intent-research.md` |
| Compaction | **Handoff instead of auto-compact** |
