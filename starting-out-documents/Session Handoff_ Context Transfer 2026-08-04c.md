# Session Handoff — Context Transfer 2026-08-04c

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Purpose:** Fresh session after Connect Slack/GitHub install slice. **Next: Teams delivery + slate 4–8.**  
**Do not auto-compact** — use this handoff + new chat.

---

## 0. What just shipped (this session / prior)

| Slice | Status |
|-------|--------|
| Sales Call Documents (00–04 PDFs) | Done · `Sales Call Documents/` |
| Champion **Cockpit** UI | Live · nav default |
| Bulk team map | Team → bulk import |
| **Connect Slack / GitHub install** | `GET /v3/oauth/status`, Slack OAuth **callback → vault**, GH install URL + webhook copy, Connections UI |

**Not this session (do next):** delivery abstraction + **Microsoft Teams**, roles, Google/SSO, tomorrow-focus persist, other chat adapters.

---

## 1. Paste prompt (next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-04c.md  ← THIS FILE
2. plans/2026-08-04_operator-champion-ux-slate.md
3. Sales Call Documents/README.md
4. deploy/oauth/README.md
5. vertical-3 twin-api OAuth routes + app-static Connections/Cockpit

Doctrine: data is currency; Actions deploy not hotspot; gas not permission-brakes.
No Linear/training. No LOC rankings. Chat = delivery; GitHub = work. ADR-012 vault only.

DONE already: cockpit, sales PDFs, bulk map, Connect Slack/GitHub (status + callback + UI).

NEXT SESSION MISSION (slate items 4–8 only):
4. Delivery abstraction + Microsoft Teams bot (Approve/Edit/Don't send Adaptive Cards; same digests)
5. Roles: champion vs member
6. Google/SSO join (when multi-tenant packaging)
7. Tomorrow focus board — persist assignments (scaffold exists on Cockpit)
8. Other chat adapters only after Teams proven

Start: smoke staging (cockpit, /v3/oauth/status, digests), then implement Teams delivery adapter behind a shared delivery interface without breaking Slack. Deploy via Actions. Handoff again when done. No auto-compact — stop ~380–400k with a new handoff if needed.
```

---

## 2. Attach list (next session)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-04c.md`  
2. `plans/2026-08-04_operator-champion-ux-slate.md`  
3. `Sales Call Documents/README.md`  
4. `Sales Call Documents/00_Leave_Behind_One_Pager.pdf`  
5. `deploy/oauth/README.md`  
6. `deploy/docker-compose.app.yml`  
7. `vertical-3/crates/twin-api/src/main.rs` (oauth_*)  
8. `vertical-3/crates/twin-delivery/` (Slack path to abstract)  
9. `vertical-3/app-static/app.js` + `index.html`  
10. `vertical-security/` (egress vault patterns)  

---

## 3. Next-session deep dive: Teams + 4–8

### 4 — Delivery abstraction + Teams

- Extract common interface used by twin-delivery (send draft DM + interactive Approve/Edit/Don't send).  
- Keep **Slack** as default adapter.  
- Add **Teams** adapter: Azure Bot + Adaptive Cards; map GitHub → AAD/Teams user id.  
- Compose/env: Teams app credentials via vault, not twin env.  
- UI: Connections “Connect Teams” status = roadmap → ready when env present.  
- **Do not break** existing Slack egress path.

### 5 — Roles champion vs member

- Cockpit/Team write actions for champion; members open own status.  
- Even on single pilot URL until full SSO.

### 6 — Google/SSO

- Identity plane only; still Connect chat + GitHub for data/delivery.

### 7 — Tomorrow focus persist

- Cockpit already suggests from conflicts/intents/digests.  
- Persist per-tenant notes/assignments (twin state JSON is fine).

### 8 — Other chat

- Only after Slack+Teams solid. WhatsApp/etc. adapter horizon.

---

## 4. Ops cheatsheet

```bash
# Smoke
curl -sf https://status.neel.world/v3/oauth/status | jq .
curl -sf https://status.neel.world/v3/oauth/slack/start | jq .ready,.authorize_url
curl -sf https://status.neel.world/v3/oauth/github/start | jq .ready,.install_url,.webhook_url

# After Slack OAuth success page: restart egress on droplet (or Actions skip_build redeploy)
# so SECRETS_FILE is reloaded
```

Staging app: **Cockpit** · **Connections** (Connect Slack / Install GitHub App).

---

## 5. Explicit session boundary

| In **next** session | Do **not** restart from scratch |
|---------------------|----------------------------------|
| Teams delivery adapter | Rebuilding cockpit/sales PDFs |
| Roles + SSO | Linear / training |
| Tomorrow focus persist | Asking for hotspot for deploy |
| | Auto-compacting this arc |

---

## 6. Document control

| Field | Value |
|-------|--------|
| Prior handoffs | `…2026-08-04.md`, `…2026-08-03b.md` |
| Slate | `plans/2026-08-04_operator-champion-ux-slate.md` |
| Compaction | **Handoff instead of auto-compact** |
