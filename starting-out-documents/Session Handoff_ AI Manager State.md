# Session Handoff — AI Manager State

**Date:** 2026-07-22  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager`  
**Purpose:** Grounded state pack for a **new chat**. Prefer this file + vertical specs over chat history.

---

## 1. Product thesis (non-negotiable)

| Stance | Detail |
|--------|--------|
| **What we are** | Permissioned **engineering context plane** + **meeting elimination** (status ledgers) |
| **Anti-Glean** | No vector search, no full-text enterprise index, no proprietary source code hosting |
| **Not Buzz** | Not multiplayer workspace / Nostr / Git forge |
| **Not Centaur** | Steal only egress credential injection |
| **Success metric** | Meetings deleted / focus time reclaimed |

---

## 2. Milestone tracker (founder finish line = M7)

| M | Meaning | Status |
|---|--------|--------|
| M0 | Strategy + ADRs | Done |
| M1 | V1 engine | Done (~85–90%) |
| M2 | V2 graph | Done (~75–80%) |
| M3 | V3 twins engine | **Done** (TC-T01–T10 green) |
| **M4** | **Sew & Show**: live sew + demo console + real Slack | **Near done** — V1+V2+V3 up; real GitHub webhook pending |
| M5 | Staging single-tenant deploy | Not started |
| M6 | Design partner weekly | Not started |
| M7 | Self-serve deployed product | Not started |

**Do not start Vertical 4** until M4 acceptance is green.

---

## 3. Monorepo layout

| Path | Role | Status |
|------|------|--------|
| `starting-out-documents/` | Strategy, ADRs, handoff, demo script, M4 plan | Living |
| `vertical-1/` | Telemetry ingest, ACL, bus | ~85–90% |
| `vertical-2/` | Context graph + projector + API | ~75–80% |
| `vertical-security/` | Egress credential proxy | MVP |
| `vertical-3/` | Twins, ledgers, veto delivery, **demo console** | MVP + M4 UI |
| `scripts/platform_sew.sh` | Cross-vertical TC-P01… | Done |

**Golden path:**

```
Source webhook → V1 → V2 graph → V3 ledger → Slack DM (egress) → veto/edit/silence → channel
```

---

## 4. Ports

| Service | Port |
|---------|------|
| V1 | **18080** |
| V2 | **18082** |
| V3 twin-api + **demo** | **18083** (`/demo/`) |
| Egress | **18090** |
| Cockroach | 26257 |
| Redis | 6379 |
| Redpanda | 19092 |

---

## 5. What works (verified)

### Engines
- V1: `cargo run -p telemetry-verify`
- V2: `cargo run -p graph-verify`
- V3: `cargo run -p twin-verify` (TC-T01–T10 **PASS**)
- Security: `cargo test` in `vertical-security/`

### M4 Sew & Show (partial)
- **Demo console:** `http://127.0.0.1:18083/demo/` — simulate PR → ledger → draft/veto/publish
- **API:** `POST /v3/demo/simulate`, `GET /v3/demo/status`, `GET /v3/demo/latest`
- **Platform sew:** `./scripts/platform_sew.sh` (embedded green with V3 only; live needs V1+V2+V3)
- **Slack:** mock default; real path ready when `USE_EGRESS_SLACK=true` + vault token

### Real Slack — **DM path verified (2026-07-22)**
- Token only in `vertical-security/secrets/dev_secrets.json` (gitignored)
- Twin: `USE_EGRESS_SLACK=true` + `EGRESS_PROXY_URL` (no bot token in twin env)
- User `U0APK7W1X99` received DM (`dm_ts` set; egress audit status 200)
- Channel publish needs bot **invited** to `C0APN754MQV` (`not_in_channel` until then)

---

## 6. Commands that should be green

```bash
cd vertical-1 && cargo run -p telemetry-verify
cd vertical-2 && cargo run -p graph-verify
cd vertical-3 && cargo run -p twin-verify
cd vertical-security && cargo test
./scripts/platform_sew.sh

# Demo UI
cd vertical-3 && RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 cargo run -p twin-api
# open http://127.0.0.1:18083/demo/
```

---

## 7. Next task (resume here)

1. **Ingest continuous / notify batched** (user feedback): bridge V1→V2 only; twin-api schedules DMs (`NOTIFY_INTERVAL_SECS=1800`, `STATUS_WINDOW_SECS=3600`).  
2. Real GitHub webhooks work via ngrok; do **not** 1:1 Slack on every delivery.  
3. Demo console = on-demand notify tool (`force_notify`).  
4. Next: M5 staging; no multi-agent V4 until M4 stable.  

---

## 8. Architecture decisions to obey

| ADR | Choice |
|-----|--------|
| 006 | Strip search/vectors |
| 007 | V2 graph on Cockroach `context_graph` |
| 010 | HybridMembership from V1 |
| 011 | Context plane, not Buzz/Centaur |
| 012 | Egress credential injection |
| 013 | V3 status twins + veto-first |

---

## 9. Context handoff protocol (~400k)

When context bloated: update this file + `Session Handoff_ Sew and Show M4.md` → stop with new-chat prompt (see Human Demo Script / plan).

### New-chat documents

1. This handoff  
2. `Plan_ Sew and Show M4.md`  
3. `Human Demo Script.md`  
4. V3 TAS  
5. ADR log  
6. vertical-2 / vertical-security / root README  

### New-chat prompt

```text
Continue AI Manager (neeljoshi18/AI-Manager). Phase Sew & Show M4.
Read Session Handoff_ AI Manager State.md and Plan_ Sew and Show M4.md first.
Do NOT start Vertical 4. Slack secrets only via egress vault.
Resume from handoff “Next task”. Autonomous until real-world secrets needed or context ~400k.
```

---

## 10. What NOT to do

- Do not re-open Buzz/Centaur/Neo4j debates  
- Do not put Slack tokens in twin env  
- Do not claim M7 “deployed product” until staging + pilot + self-serve  
- Do not skip sew for shiny new verticals  

---

*End of handoff. Prefer git files over conversation memory.*
