# Session Handoff — Context Transfer

**Date:** 2026-07-23  
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
| M4 | Sew & Show: demo, real Slack, GitHub live, **batched notify** | **Done enough** |
| **M5** | Staging product: host, TLS, OAuth, GitHub App, product UI | **In progress** |
| M6 | Design partner: Jira/Linear, Slack inbound, conflict v0 | Planned |
| M7 | Self-serve multi-tenant | Planned |
| M8+ | Intent graph V4, browser opt-in, richer agents | Roadmap only |

**Do not** invent Centaur/Buzz clones. **Do not** 1:1 webhook → Slack DM (ADR-014).

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

**User confirmed:** real PR → Slack DM (then spam fixed via batching).

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

## 6. Plan to follow until deployment (M5 cycle)

**Primary plan file:** `plans/2026-07-23_demo-to-product-m5.md`  
**Execution log:** `plans/2026-07-23_m5-and-product-ui.md`  
**Living backlog:** `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`

### Ordered work until “deployed staging”

1. **Local reliability** — `dev_up` solid; wake-from-sleep documented (done).  
2. **Product UI** — B&W shell at `/app/` (done v1); continue polish (connections last-event age, empty states).  
3. **M5 host** — single VPS or Fly; `deploy/` compose for infra; TLS reverse proxy to 18083 (+ 18080 for webhooks).  
4. **Containerize** — twin-api Dockerfile exists; add V1/V2/egress images + multi-service compose.  
5. **Slack OAuth** — install link; bot token → egress vault only.  
6. **GitHub App** — replace ngrok for real tenants.  
7. **Onboarding wizard** in UI: tenant → Slack → GitHub → shadow → first digest.  
8. **Definition of staging done:** stranger opens HTTPS URL, connects Slack+GitHub, gets ≤1 DM per notify window, can veto/publish.

### After staging (do not jump early)

- Jira/Linear connector productization  
- Slack channel ingest (metadata + short text)  
- Intent classification v0 (rules → graph) — **not** full agent OS  
- Conflict cards + thin monitor workers  
- Design partner (M6)

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
[x] Finish multi-service Docker compose for V1+V2+V3+egress
[x] Staging HTTPS path scaffold (Caddy profile tls + deploy/README)
[x] Connections UI: last event age from V1 when up
[x] GitHub App + Slack OAuth scaffolding (manifests; stop for secrets)
[ ] Host + public URL (need human host/DNS choice)
[x] OAuth start endpoints (501 until secrets) + live onboarding steps API
[ ] OAuth callback → vault write when Slack/GitHub credentials exist
[x] Onboarding wizard polish in /app (server-driven steps)
[ ] Keep ADRs; append Interaction Log on decisions
[ ] Save any new plan under plans/YYYY-MM-DD_slug.md
```

**Autonomous until:** human secrets (OAuth/App), hosting account, or real deploy credentials.

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

Mission: continue M5 “demo → product → staging deploy” autonomously.
- Product UI is at :18083/app/ (light B&W Fish-inspired); lab at /demo/
- Ingest continuous; Slack notify BATCHED (ADR-014). Bridge = V1→V2 only.
- Slack secrets only via egress vault (ADR-012). No Buzz/Centaur clones (ADR-011).
- No silent private DM wiretap (ADR-015).
- Save new plans under plans/YYYY-MM-DD_slug.md
- Commit and push cleanly when slices complete.
- Stop only when you need human secrets (Slack OAuth, GitHub App, hosting) or confirmation.

Next tasks (from handoff §7): multi-service Docker compose; staging HTTPS path; Connections last-event age; OAuth/App scaffolding; onboarding polish.

Start by confirming you read the handoff + M5 plan, then implement the next highest-value slice without waiting for me unless blocked.
```

---

*End of context transfer. Prefer git files over conversation memory.*
