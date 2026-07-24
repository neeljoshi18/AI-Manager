# Session Handoff — Context Transfer

**Date:** 2026-07-23 (updated **2026-07-24**)  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager` (branch `main`)  
**Purpose:** Full handoff for a **new chat**. Prefer this file + listed plans over compacted history.

---

## 1. Product thesis (non-negotiable)

| Stance | Detail |
|--------|--------|
| **What we are** | Permissioned **engineering context plane** + **meeting elimination** (status ledgers) |
| **Anti-Glean** | No vector search, no full-text index, no proprietary source hosting |
| **Not Buzz** | Not multiplayer workspace / Nostr / Git forge |
| **Not Centaur** | Steal **egress credential inject only** — not K8s agent OS |
| **Success** | Meetings deleted / focus time reclaimed — not engagement rankings |

**Golden path:**

```
Sources (GitHub…) → V1 ingest (ACL) → V2 graph → V3 status twin → Slack DM/channel (egress, veto-first, BATCHED notify)
```

---

## 2. Milestone status

| M | Meaning | Status |
|---|--------|--------|
| M0–M3 | Strategy + V1/V2 engines + V3 twins | **Done** |
| M4 | Sew & Show: demo, real Slack, GitHub live, **batched notify** | **Done** |
| **M5** | Staging product: host, TLS, GitHub App, product UI, bridge | **Done enough** (`status.neel.world`) |
| **M6** | Multi-member beta: connectors, intent/conflict v0, thin agents | **Next** |
| M6.5 | Learning window 10–14d + training-pair export | After first partner |
| M7 | Model Router + customer-prem SLM (ADR-016) + self-serve | After shadow gold |
| M8+ | Richer agents, browser opt-in | Roadmap only |

**Do not** invent Centaur/Buzz clones. **Do not** 1:1 webhook → Slack DM (ADR-014).  
**Do not** train local models before multi-person digests + shadow gold (ADR-016).

---

## 3. What works (verified in prior sessions)

| Capability | Notes |
|------------|--------|
| V1 webhooks GitHub | HMAC, tenant `ten_github`, ngrok path tested |
| V2 graph | Project API + neighborhood; HybridMembership with V1 |
| V3 twins | Compile, veto/edit/publish, shadow, confidence tiers |
| Egress Slack | Token **only** in `vertical-security/secrets/dev_secrets.json` |
| Batched notify | `NOTIFY_INTERVAL_SECS=1800`, `STATUS_WINDOW_SECS=3600`, bridge = ingest only |
| Product UI | **http://127.0.0.1:18083/app/** — light B&W (Fish Audio–inspired) |
| Lab UI | **http://127.0.0.1:18083/demo/** |
| Plans folder | `plans/` dated snapshots |
| Deploy scaffold | `deploy/docker-compose.platform.yml`, `deploy/docker-compose.app.yml`, Dockerfiles for V1/V2/V3/egress, Caddy TLS profile, `deploy/oauth/` |
| Dev ops | `./scripts/dev_up.sh` / `dev_down.sh` |
| Connections last-event | V1 `/healthz` → twin `/v3/demo/status` → `/app/` Connections |
| Staging URL | **https://status.neel.world/app/** (DO VPS always-on) |
| Bridge | Always-on container: V1→V2 + twin upsert (`SLACK_USER_MAP`) — never Slack |

**User confirmed:** real PR → Slack DM (then spam fixed via batching); staging live.

---

## 4. Ports

| Service | Port |
|---------|------|
| V1 ingestion | 18080 |
| V2 graph-api | 18082 |
| V3 twin-api + `/app` + `/demo` | 18083 |
| Egress proxy | 18090 |
| Cockroach (compose) | 26257 |
| Redis | 6379 |
| Redpanda | 19092 |

---

## 5. Env knobs (V3 notify policy)

| Env | Default | Role |
|-----|---------|------|
| `STATUS_WINDOW_SECS` | 3600 | Ledger period align |
| `NOTIFY_INTERVAL_SECS` | 1800 | Min between DMs per twin |
| `COMPILE_INTERVAL_SECS` | 1800 | Scheduler tick; `0` = off |
| `NOTIFY_ON_COMPILE` | false | HTTP compile does not spam Slack |
| `USE_EGRESS_SLACK` | — | true for real Slack via proxy |
| `EGRESS_PROXY_URL` | http://127.0.0.1:18090 | |
| `V2_BASE_URL` | http://127.0.0.1:18082 | Overlay graph source |

**Bridge** `scripts/github_live_bridge.py`: **V1→V2 only** — never compile/DM.

---

## 6. Plan to follow (post-M5 → M6 beta → local model)

**Vision / sequence:** `plans/2026-07-24_onprem-model-and-agents.md` + **ADR-016**  
**Living backlog:** `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`  
**M5 parent plans:** `plans/2026-07-23_demo-to-product-m5.md`, `…_m5-multiservice-compose.md`

### Sequence lock

```
M5 staging (done enough) → M6 multi-member + thin agents → design-partner shadow 10–14d
  → gold pairs → M7 Model Router + customer-prem SLM
```

### Next engineering (M6 — beta-ready)

1. Multi-person Slack map (2+ humans)  
2. Intent v0 (rules) + conflict v0  
3. Thin monitors (ingest health, graph delta, conflict cards)  
4. Linear **or** Jira productized; Slack channel metadata  
5. Metrics: veto rate, DMs, empty windows  
6. Design-partner one-pager + learning-window playbook  

### After first partner shadow (not now)

- Model Router (`rules` | `cloud` | `local`)  
- Ollama/vLLM recipe on customer box  
- Distill/LoRA on **approved digests + intent labels only** (not raw source)

### Intent classification (summary for next agent)

- **Fact** = webhook event; **intent** = claim about purpose (SHIP / BLOCKED / …).  
- Pipeline: extract → type → attach to V2 → conflict detect → surface (batched Slack / UI).  
- v0 deterministic; LLM optional via egress for prose only.  
- Private human↔human DMs: **no silent wiretap** (ADR-015); bot-mediated / opt-in only.

### UX pillars (stand by these)

- Linear-like calm density; Vercel-like connection health; Slack for delivery.  
- **Light B&W** product chrome (Fish Audio developer portal inspiration).  
- Status as first-class objects; hide raw JSON in Lab.  
- Health = last successful ingest when possible, not only process up/down.

---

## 7. Next concrete tasks for the new session

```
[x] Multi-service Docker + staging HTTPS (status.neel.world)
[x] GitHub App + Slack vault + always-on bridge
[x] ADR-016 + on-prem model / learning-window vision docs
[ ] M6: multi-person twins + intent/conflict v0 + thin agents
[ ] Design-partner learning window playbook (10–14d)
[ ] M7: Model Router + customer-prem SLM (only after shadow gold)
[ ] OAuth callback → vault write (self-serve polish)
```

**Autonomous until:** multi-member mapping product decisions, design-partner identity, or new connector secrets.

---

## 8. Security rules (never break)

- `SLACK_BOT_TOKEN` / `GITHUB_TOKEN` only in egress vault file (gitignored).  
- Never put long-lived tokens in twin/worker env.  
- No god-mode SQL into `context_graph` from V3.  
- No LOC rankings / surveillance product surfaces.

---

## 9. Commands

```bash
# Local product stack
./scripts/dev_up.sh
open http://127.0.0.1:18083/app/

# Stop
./scripts/dev_down.sh

# Verify batteries
cd vertical-1 && cargo run -p telemetry-verify
cd vertical-2 && cargo run -p graph-verify
cd vertical-3 && cargo run -p twin-verify

# Platform sew
./scripts/platform_sew.sh
SEW_MODE=live ./scripts/platform_sew.sh

# Infra only
docker compose -f deploy/docker-compose.platform.yml up -d

# Multi-service app stack (embedded demos / first VPS)
docker compose -f deploy/docker-compose.app.yml up -d --build

# HTTPS (set DOMAIN first)
# DOMAIN=status.example.com docker compose -f deploy/docker-compose.app.yml --profile tls up -d --build
```

---

## 10. Recent commits (reference)

- `3fafd21` — twin-api Dockerfile + onboarding checklist  
- `596248a` — B&W UI + platform compose  
- `714e4c9` — plans/ + product shell + dev_up  
- `bb50106` — product roadmap + ADR-014/015  
- `1cb28c1` — V3 sew, batch notify, GitHub bridge  

---

## 11. Files to attach in the new chat (minimum set)

**Required (paste or @ these):**

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-07-23.md` ← **this file**  
2. `plans/2026-07-23_demo-to-product-m5.md`  
3. `plans/2026-07-23_m5-and-product-ui.md`  
4. `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`  
5. `starting-out-documents/Architecture Decision Log_ Pivotal Choices.md`  
6. `README.md`  

**Strongly recommended:**

7. `vertical-3/Technical Architecture Specification_ Vertical 3.md`  
8. `starting-out-documents/Interaction Log_ Product Decisions.md`  
9. `deploy/README.md`  
10. `plans/README.md`  

**Optional if working connectors:**

11. `vertical-2/README.md`  
12. `vertical-security/README.md`  
13. `starting-out-documents/GitHub Webhook Setup_ Local.md`  

---

## 12. Prompt to paste into the next session

Copy everything below the line into a new chat (with the files above attached):

---

```text
You are continuing the AI Manager monorepo (private GitHub neeljoshi18/AI-Manager, branch main).

Read these ground-truth files first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-07-23.md
2. plans/2026-07-23_demo-to-product-m5.md
3. plans/2026-07-23_m5-and-product-ui.md
4. starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md
5. starting-out-documents/Architecture Decision Log_ Pivotal Choices.md
6. README.md
7. vertical-3/Technical Architecture Specification_ Vertical 3.md (if attached)

Mission: continue M6 multi-member beta path (agents/intent/conflict), not local model training yet (ADR-016).
- Staging: https://status.neel.world/app/ — Product UI light B&W; lab at /demo/
- Ingest continuous; Slack notify BATCHED (ADR-014). Bridge = V1→V2 + twin upsert only (never DM).
- Slack secrets only via egress vault (ADR-012). No Buzz/Centaur clones (ADR-011).
- No silent private DM wiretap (ADR-015).
- Local SLM / Model Router only after multi-person digests + learning-window gold (ADR-016).
- Save new plans under plans/YYYY-MM-DD_slug.md
- Commit and push cleanly when slices complete.

Next tasks (handoff §7): multi-person twins; intent/conflict v0; thin agents; design-partner playbook.

Read handoff + plans/2026-07-24_onprem-model-and-agents.md + Product Roadmap; implement highest-value M6 slice unless blocked.
```

---

*End of context transfer. Prefer git files over conversation memory.*
