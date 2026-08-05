# Session Handoff — Context Transfer 2026-08-05

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Purpose:** After slate **4–8** (delivery abstraction + Teams, roles, tomorrow focus, SSO scaffold).  
**Do not auto-compact** — use this handoff + new chat.

---

## 0. What just shipped (this session)

| Slice | Status |
|-------|--------|
| Shared **`DeliveryClient`** interface | Done · `twin-delivery` (`delivery.rs`) |
| Slack remains **default** adapter | Unbroken · `DELIVERY_ADAPTER=slack` |
| **Teams** adapter + Adaptive Cards | Done · `teams.rs`, egress `teams_bot`, `/v3/teams/messages` |
| Connect Teams UI + oauth status | Done · Connections · `GET /v3/oauth/teams/start` |
| Roles champion vs member | Done · `GET/PUT /v3/tenants/{id}/roles` + Connections UI |
| Tomorrow focus **persist** | Done · `GET/PUT …/tomorrow_focus` + Cockpit pin |
| Google/SSO | Scaffold only · `oauth/status.sso` roadmap |
| Other chat adapters | Explicitly **not** started (after Slack+Teams proven) |

---

## 1. Paste prompt (next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

Read first:
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-05.md
2. plans/2026-08-04_operator-champion-ux-slate.md
3. deploy/oauth/README.md (Teams section)

Doctrine: Actions not hotspot; gas; no Linear/training; vault for tokens; no LOC rankings.
Chat = delivery; GitHub = work. ADR-012 vault only. Do not break Slack default.

DONE: cockpit, sales PDFs, bulk map, Connect Slack/GitHub, DeliveryClient + Teams adapter,
roles API, tomorrow focus persist, SSO scaffold status.

NEXT (pick by pilot need):
- White-glove Teams: human Azure Bot + vault TEAMS_BOT_TOKEN; smoke Adaptive Card DM
- Role enforcement on write APIs (soft header / subject) beyond storage
- Google/SSO when multi-tenant packaging starts
- Other chat only after Teams pilot proven
- Sales PDF refresh for Teams status if calling this week

Start: smoke /v3/oauth/status (teams + sso fields), cockpit pin, roles. Deploy via Actions.
Handoff if context high — no auto-compact.
```

---

## 2. Ops / smoke

```bash
curl -sf https://status.neel.world/v3/oauth/status | jq .teams,.sso,.delivery_adapter
curl -sf https://status.neel.world/v3/oauth/teams/start | jq .
# Tomorrow focus (embedded)
curl -sf -X PUT https://status.neel.world/v3/tenants/ten_github/tomorrow_focus \
  -H 'content-type: application/json' \
  -d '{"items":[{"kind":"pin","text":"smoke","why":"test"}],"note":"handoff"}' | jq .
curl -sf https://status.neel.world/v3/tenants/ten_github/roles | jq .
```

**Enable Teams (human + vault):**
1. Vault `TEAMS_BOT_TOKEN`
2. Env: `TEAMS_APP_ID`, `DELIVERY_ADAPTER=teams`, `USE_EGRESS_TEAMS=true`
3. Restart egress + twin-api
4. Map `teams_user_id` on team members

**Keep Slack pilots:** leave `DELIVERY_ADAPTER` unset or `slack`.

---

## 3. Key paths

| Area | Path |
|------|------|
| Delivery trait | `vertical-3/crates/twin-delivery/src/delivery.rs` |
| Teams adapter | `…/teams.rs` |
| Slack adapter | `…/slack.rs` (still default) |
| Worker | `…/worker.rs` uses `DeliveryClient` + `resolve_chat_user_id` |
| Egress tool | `vertical-security/config/tool_registry.yaml` → `teams_bot` |
| Manifest | `deploy/oauth/teams-app-manifest.json` |
| UI | `vertical-3/app-static/` Connections + Cockpit pin + Roles |

---

## 4. Explicit session boundary

| In **next** session | Do **not** restart |
|---------------------|--------------------|
| Live Teams white-glove with real Azure bot | Rebuilding delivery abstraction |
| SSO packaging | Linear / training / LOC |
| Hard role gates on every write | Asking for hotspot for deploy |
| | Auto-compacting this arc |

---

## 5. Document control

| Field | Value |
|-------|--------|
| Prior | `…2026-08-04c.md` |
| Slate | `plans/2026-08-04_operator-champion-ux-slate.md` (4–7 done / 6 scaffold / 8 later) |
| Compaction | **Handoff instead of auto-compact** |
