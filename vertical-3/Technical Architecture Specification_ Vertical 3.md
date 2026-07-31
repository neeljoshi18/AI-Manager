# Technical Architecture Specification: Vertical 3  
## Status Twins, Ledgers & Veto-First Delivery

**Status:** Ground-truth specification (implementation pending)  
**Date:** 2026-07-22  
**Related:** ADR-011, ADR-012, ADR-013; Vertical 1 & 2 TAS; Competitive Landscape (Buzz/Centaur); Session Handoff  

**Implementation must not invent alternate product scope without a new ADR.**

---

## 1. Executive Summary & Core Invariants

### 1.1 Scope & Purpose

**Vertical 3** is the **user-visible product layer** of the Autonomous AI Manager platform. It continuously compiles a **Status Ledger** for each engineer (and optionally each team) from the Vertical 2 Organizational Context Graph, delivers that ledger **privately first** (Slack DM), respects **confidence tiers** and **human veto**, and only then publishes to a team channel — replacing daily standup theater.

Vertical 1 and Vertical 2 are necessary infrastructure. **Vertical 3 is where the pitch becomes real.**

Vertical 3 does **not** host proprietary source code, full document bodies, vector indices, multiplayer coding workspaces (Buzz), or secure multiplayer agent kernels (Centaur). It **does** require Centaur-style **egress credential injection** for outbound Slack (and similar) writes.

### 1.2 System Invariants (Non-Negotiable Guarantees)

1. **ACL never bypassed.** Every graph or event read uses Vertical 2 / Vertical 1 APIs with `tenant_id` + `global_user_id` (groups resolved live via HybridMembership semantics). The twin process has **no god-mode SQL** access to private `context_graph` rows.
2. **Veto-first.** No public channel status post without a developer opportunity to edit or veto (silence = consent **only** for Medium tier per §5).
3. **Confidence tiers.** High / Medium / Blocker drive delivery policy. Never invent commits or work items.
4. **Evidence-backed.** Every ledger item references graph node/edge IDs and/or V1 `event_id`s.
5. **Egress-only outbound secrets.** `SLACK_BOT_TOKEN` and similar live only in the egress vault; twin processes see tool name + allowlisted host only.
6. **No upward coupling.** Vertical 1 and Vertical 2 must not depend on Vertical 3. Vertical 3 depends on V2 contracts + egress.
7. **Metadata posture.** Summaries are short text derived from titles/states; never store PR diffs or full message bodies.
8. **Tenant isolation.** All tables keyed by `tenant_id`; fail closed on missing context.
9. **No surveillance product.** No individual LOC rankings or “productivity scores” as product surfaces.

### 1.3 What Vertical 3 Does / Does Not

| Does | Does not |
|------|----------|
| Own twin registry, ledger snapshots, draft/veto state, publish records | Accept GitHub/Jira webhooks (V1 only) |
| Call ACL-safe V2 Graph API (+ optional V1 query) | Own organizational graph SoT (V2 only) |
| Send Slack DM + channel posts via egress-proxy | Ship Centaur K8s sandboxes or Buzz Nostr/Git |
| Deterministic confidence first; optional LLM prose only | Index docs / vectors / full-text (anti-Glean) |

### 1.4 Relationship to Other Verticals

| Concern | Owner |
|---------|--------|
| Webhooks, HMAC, canonical events, identity membership | Vertical 1 |
| Context graph, multi-hop, blockers API | Vertical 2 |
| Outbound credential inject | `vertical-security` |
| Twin, ledger, DM, veto, publish | **Vertical 3** |
| Optional twin-to-twin negotiation | V3 phase-2 (still human-gated for external posts) |

```
V1 events ──► V2 graph-api (ACL QueryContext)
                      │
                      ▼
              ┌───────────────────────┐
              │ VERTICAL 3            │
              │ twin-compiler         │
              │ confidence scorer     │
              │ delivery (DM/veto)    │
              │ publisher             │
              └───────────────────────┘
                      │
                      ▼ egress :18090
                 Slack Web API
```

---

## 2. Competitive Landscape Constraints

| Player | They optimize | V3 response |
|--------|---------------|-------------|
| **Glean** | Horizontal search engagement | Never index docs; **publish structured status** from graph evidence |
| **Buzz** | Shared agent room + Git forge | Stay in **existing Slack**; reduce meetings; no workspace core |
| **Centaur** | Secure multiplayer agents + sandboxes | **Reuse egress secret inject**; no sandbox swarm as product |
| **Geekbot-class** | Empty standup forms | **Draft from telemetry**, not nag-only |
| **LinearB-class** | Ranking / dashboards | Team ledger + blockers; **no individual stack-rank product** |

**ADR-011 remains law:** AI Manager is the permissioned engineering context + meeting elimination layer.

---

## 3. Technical Stack Selection & Trade-Off Matrix

### 3.1 Selected Architecture

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language / HTTP | **Rust + Axum** | Same as V1/V2; ops consistency |
| Twin state store | **CockroachDB** database `status_twins` (isolated, like `context_graph`) | ACID ledger versions + veto state; no new DB product |
| Schedule / jobs | **Tokio** periodic compile + Redis locks; optional Redpanda later | Avoid Temporal until multi-step workflows demand it |
| Cache / timers | **Redis** (existing V1 stack) | Draft locks, veto deadlines, rate limits |
| LLM (optional) | Pluggable provider via **egress proxy**; prompts grounded in ledger JSON | Structure first, prose second; no cost explosion |
| Slack | Events API + Web API via **egress-proxy** | Centaur pattern for bot token |
| Identity | V1 `global_user_id` + `slack_user_map` in `status_twins` | No second identity plane |
| Bind port | **`:18083`** | Avoid V1 `:18080`, V2 `:18082`, egress `:18090` |

### 3.2 Explicit Rejections

| Rejected | Why |
|----------|-----|
| Neo4j / new graph DB in V3 | Graph already owned by V2/Cockroach |
| Full Centaur K8s sandboxes | Off-mission (ADR-011) |
| Buzz Nostr workspace | Off-mission; stay in customer Slack |
| Vector search / RAG over docs | Anti-Glean strip (ADR-006) |
| Env-var Slack tokens in twin | ADR-012; TC-T07 |
| Individual LOC rankings | Product ethics |
| God-mode SQL into `context_graph` | ACL bypass risk |

### 3.3 Stack Comparison (summary)

| Architectural Layer | Alternatives | Selected | Trade-off |
| :--- | :--- | :--- | :--- |
| Twin SoT | Postgres-only, Dynamo | **Cockroach `status_twins`** | Reuses cluster; SQL ops already known |
| Workflow engine | Temporal, custom Kafka | **Tokio + CRDB state** | Simpler MVP; revisit if long sagas |
| Delivery channel | Email, Teams-first | **Slack first** | Mid-market eng default; Teams later |
| Prose generation | Always LLM, never LLM | **Rules first, LLM optional** | Avoid hallucination + cost |

---

## 4. Subsystem Architecture & Pipeline Specification

### 4.1 Twin Registry

- One **PersonTwin** per `(tenant_id, global_user_id)`
- Optional **TeamTwin** per `(tenant_id, team_node_id)` for rollups
- Config: channel targets, timezone, compile cadence, `shadow_until`, `high_auto_publish`, enabled flag

### 4.2 Ledger Compiler (`twin-compiler`)

**Inputs (ACL-scoped as the twin’s person):**

- V2 `GET /v2/tenants/{t}/neighborhood` (or nodes/{person}/neighborhood) with hops 2–3  
- V2 blockers endpoint  
- V2 entity state for open PRs/tickets  
- Optional: V1 recent events for evidence enrichment  

**Output:** `StatusLedger` JSON stored as an immutable `ledger_snapshot` row.

**Cadence (v1 + M4 batching):**

| Concern | Policy |
|---------|--------|
| **Ingest** | Continuous — accept all GitHub/Jira webhooks into V1→V2 (high volume OK) |
| **Ledger period** | Aligned wall-clock bucket for ledger_id; rolling activity lookback uses same `STATUS_WINDOW_SECS` (default **86400** = 24h pilot) |
| **Compile tick** | Tokio scheduler every `COMPILE_INTERVAL_SECS` (default **1800** = 30m); `0` disables |
| **Slack DM** | At most once per twin per `NOTIFY_INTERVAL_SECS` (default **1800**); **not** on every webhook |
| **On-demand** | Demo console / `force_notify=true` on compile — separate “check now” tool |

**Never** 1:1 map “GitHub delivery → Slack DM”. Bridge processes project graph only; twin-api owns notify.

**Idempotency:** unique `(tenant_id, twin_id, period_start, period_end)`. Recompile may update open draft text; DM only when notify budget allows.

### 4.3 Confidence Scorer (Deterministic v1)

| Signal | Contribution |
|--------|----------------|
| Merged/closed PR or closed ticket in window with graph evidence | **High** |
| Open PR with commits/activity in window | **Medium** |
| Open `BLOCKS` / `BLOCKED_BY` edge involving person | **Blocker** |
| No graph activity in window | **Medium empty** — honest “no code signals”; never fabricate |

**Rollup rules:**

- If any item or open blocker is **Blocker** → rollup **Blocker**
- Else if any **High** and no Medium-only requirement conflict → prefer **High** when all substantive items are High
- Else → **Medium**
- Empty activity → **Medium** with empty items list (honest silence)

**LLM role (optional, phase-1b):** Turn structured items into 3–5 bullet prose. **Never invent items** not present in ledger JSON. If LLM fails, use structured bullets only.

### 4.4 Delivery Service (Veto State Machine)

```
SHADOW ──(shadow_until elapsed)──► COMPILED
COMPILED ──► DM_SENT (PENDING)
PENDING ──edit──► EDITED
PENDING ──veto──► VETOED
PENDING ──timeout silence (Medium)──► PUBLISH_QUEUED
EDITED ──confirm or timeout──► PUBLISH_QUEUED
HIGH + high_auto_publish ──► PUBLISH_QUEUED (optional short DM)
BLOCKER ──► FORCE_HUMAN (no auto channel post)
PUBLISH_QUEUED ──► PUBLISHED | PUBLISH_FAILED
```

**Slack interactions:**

- DM with buttons: **Publish as-is / Edit / Veto**
- Edit: modal or thread reply → `edited_text`
- Veto: terminal; increment `veto_total`
- Timeouts: Redis key and/or CRDB `veto_deadline` polled by delivery worker

### 4.5 Publisher

- Posts to configured team channel via egress tool `slack_api`
- Writes `publish_record` with channel, Slack `ts`, ledger_id, body hash
- **Exactly-once publish intent:** `UNIQUE (tenant_id, ledger_id)` on `publish_record` (TC-T08)

### 4.6 Optional Twin Negotiation (Phase-2 — out of MVP)

- Twin A may query Twin B’s **team-visible** blockers about a shared edge
- Still **no external post** without human gates
- Do **not** build a full multi-agent OS

---

## 5. Delivery Policy by Confidence Tier

| Tier | DM? | Silence after deadline | Auto channel publish | Notes |
|------|-----|------------------------|----------------------|-------|
| **High** | Optional short notice | N/A if auto | **Yes if** `high_auto_publish=true` (default **false** first 10 days) | Prefer trust-building: start with DM even for High |
| **Medium** | **Required** | **Yes → publish** (opt-out) | After deadline or explicit Publish | Silence = consent for medium only |
| **Blocker** | **Required** | **No auto publish** | Only explicit Publish after human action | Force human intervention |
| **Shadow** | **No DM** | N/A | **No** | Silent compile N days; store for quality review |

**Default product flags (v1):**

| Flag | Default |
|------|---------|
| `shadow_mode_days` | `10` |
| `high_auto_publish` | `false` |
| `medium_veto_window` | `2h` |
| `blocker_veto_window` | `24h` (still no auto publish) |

---

## 6. Data Model (CockroachDB `status_twins`)

```sql
CREATE DATABASE IF NOT EXISTS status_twins;

CREATE TABLE IF NOT EXISTS status_twins.twin (
    tenant_id         STRING NOT NULL,
    twin_id           STRING NOT NULL,  -- twin:person:gu_… or twin:team:…
    twin_kind         STRING NOT NULL,  -- person|team
    subject_id        STRING NOT NULL,  -- global_user_id or team node_id
    display_name      STRING NOT NULL DEFAULT '',
    timezone          STRING NOT NULL DEFAULT 'UTC',
    channel_id        STRING NOT NULL DEFAULT '',  -- Slack channel for publish
    shadow_until      TIMESTAMPTZ NULL,
    high_auto_publish BOOL NOT NULL DEFAULT false,
    enabled           BOOL NOT NULL DEFAULT true,
    config_json       JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, twin_id)
);

CREATE TABLE IF NOT EXISTS status_twins.slack_user_map (
    tenant_id       STRING NOT NULL,
    global_user_id  STRING NOT NULL,
    slack_user_id   STRING NOT NULL,
    slack_team_id   STRING NOT NULL DEFAULT '',
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, global_user_id),
    UNIQUE (tenant_id, slack_user_id)
);

CREATE TABLE IF NOT EXISTS status_twins.ledger_snapshot (
    tenant_id          STRING NOT NULL,
    ledger_id          STRING NOT NULL,
    twin_id            STRING NOT NULL,
    period_start       TIMESTAMPTZ NOT NULL,
    period_end         TIMESTAMPTZ NOT NULL,
    confidence_rollup  STRING NOT NULL,  -- high|medium|blocker
    ledger_json        JSONB NOT NULL,
    graph_as_of        TIMESTAMPTZ NOT NULL,
    compiled_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, ledger_id),
    UNIQUE (tenant_id, twin_id, period_start, period_end)
);

CREATE TABLE IF NOT EXISTS status_twins.draft_delivery (
    tenant_id        STRING NOT NULL,
    draft_id         STRING NOT NULL,
    ledger_id        STRING NOT NULL,
    twin_id          STRING NOT NULL,
    status           STRING NOT NULL,
    -- shadow|pending|edited|vetoed|publish_queued|published|expired|force_human
    slack_dm_channel STRING NOT NULL DEFAULT '',
    slack_dm_ts      STRING NOT NULL DEFAULT '',
    draft_text       STRING NOT NULL DEFAULT '',
    edited_text      STRING NULL,
    veto_deadline    TIMESTAMPTZ NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, draft_id),
    UNIQUE (tenant_id, ledger_id)
);

CREATE TABLE IF NOT EXISTS status_twins.publish_record (
    tenant_id    STRING NOT NULL,
    publish_id   STRING NOT NULL,
    ledger_id    STRING NOT NULL,
    draft_id     STRING NOT NULL,
    channel_id   STRING NOT NULL,
    slack_ts     STRING NOT NULL DEFAULT '',
    body_hash    STRING NOT NULL,
    published_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, publish_id),
    UNIQUE (tenant_id, ledger_id)
);

CREATE TABLE IF NOT EXISTS status_twins.compile_run (
    tenant_id   STRING NOT NULL,
    run_id      STRING NOT NULL,
    twin_id     STRING NOT NULL,
    status      STRING NOT NULL, -- ok|error|skipped_shadow
    error_text  STRING NOT NULL DEFAULT '',
    started_at  TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NULL,
    PRIMARY KEY (tenant_id, run_id)
);
```

### 6.1 StatusLedger JSON Contract

```json
{
  "tenant_id": "ten_acme",
  "person_id": "gu_alice",
  "period": { "start": "2026-07-21T00:00:00Z", "end": "2026-07-22T00:00:00Z" },
  "confidence_rollup": "medium",
  "items": [
    {
      "kind": "pr",
      "resource_id": "acme/app/pr/7",
      "node_id": "pr:acme/app/pr/7",
      "summary": "Opened PR #7: fix auth race",
      "confidence": "medium",
      "evidence_refs": ["event:evt_123", "edge:authored:..."]
    }
  ],
  "open_blockers": [
    {
      "node_id": "issue:...",
      "summary": "Blocked on API key rotation",
      "confidence": "blocker",
      "evidence_refs": ["edge:blocks:..."]
    }
  ],
  "graph_as_of": "2026-07-22T08:00:00Z",
  "compiled_at": "2026-07-22T08:00:01Z"
}
```

---

## 7. API Surface

### 7.1 Internal twin-api (`:18083`)

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/healthz` `/readyz` `/metrics` | Ops |
| `POST` | `/v3/tenants/{t}/twins` | Upsert twin config |
| `GET` | `/v3/tenants/{t}/twins/{twin_id}` | Get twin |
| `POST` | `/v3/tenants/{t}/twins/{twin_id}/compile` | On-demand compile |
| `GET` | `/v3/tenants/{t}/ledgers/{ledger_id}` | Fetch ledger snapshot |
| `GET` | `/v3/tenants/{t}/drafts/{draft_id}` | Draft status |
| `POST` | `/v3/tenants/{t}/drafts/{draft_id}/veto` | Programmatic veto (tests) |
| `POST` | `/v3/tenants/{t}/drafts/{draft_id}/publish` | Force publish path (tests/admin) |
| `POST` | `/v3/slack/interactions` | Slack interactivity endpoint |
| `POST` | `/v3/slack/events` | Slack events (optional) |

Production routes require service auth (HMAC or shared token). `SKIP_AUTH` for local only (same pattern as V1).

### 7.2 Vertical 2 Contracts Required

| V2 capability | V3 use |
|---------------|--------|
| Neighborhood of person | Ledger items |
| Blockers listing | Blocker rollup |
| Entity state | PR/ticket lifecycle |
| HybridMembership / QueryContext | ACL safety |

If an endpoint is missing or incomplete, **extend Vertical 2** rather than reading raw SQL from V3.

### 7.3 Egress Tools Required

Extend `vertical-security/config/tool_registry.yaml` (if not already present):

```yaml
slack_api:
  hosts:
    - slack.com
    - www.slack.com
  secret_ref: SLACK_BOT_TOKEN
  # inject Authorization: Bearer …
```

**Never** put `SLACK_BOT_TOKEN` in the twin process environment.

---

## 8. Security

1. Twin processes hold **no** long-lived third-party tokens.
2. Fail closed if egress is unavailable (queue publish; do not fall back to env vars).
3. Compile **as the person**: service identity still filters by that user’s groups. Never compile personal DMs with admin superuser groups.
4. Team rollups: only public + groups the **requesting** manager is in (document separately when implemented).
5. Audit: egress proxy logs tool/host/status; `status_twins` logs veto/publish.
6. Keep proxy response redaction enabled for Slack API responses.

---

## 9. Repository & Runtime Layout

```
vertical-3/
├── Technical Architecture Specification_ Vertical 3.md   # this document
├── README.md
├── Cargo.toml                      # workspace (implementation phase)
├── crates/
│   ├── twin-core/                  # domain: ledger, confidence, state machine
│   ├── twin-compiler/              # V2 client → ledger_snapshot
│   ├── twin-delivery/              # DM + veto worker + Slack interactivity
│   ├── twin-api/                   # Axum :18083
│   └── twin-verify/                # TC-T01… battery
├── migrations/cockroach/
│   └── 001_init.sql
├── proto/                          # optional StatusLedger.proto later
└── scripts/
    ├── smoke_v3.sh
    └── sew_e2e.sh                  # V1→V2→V3 golden path
```

**Isolation rule:** V3 code lives only under `vertical-3/`. No nested “V3 inside V1” crates.

**Runtime env (shares V1 infra):**

| Variable | Purpose |
|----------|---------|
| `RUNTIME_MODE` | `embedded` \| `production` |
| `COCKROACH_URL` | `status_twins` database |
| `V2_BASE_URL` | e.g. `http://127.0.0.1:18082` |
| `EGRESS_PROXY_URL` | e.g. `http://127.0.0.1:18090` |
| `REDIS_URL` | locks / timers |
| `SKIP_AUTH` | local only |
| `BIND_ADDR` | default `0.0.0.0:18083` |
| `STATUS_WINDOW_SECS` | ledger period align + rolling activity lookback (default 86400) |
| `NOTIFY_INTERVAL_SECS` | min seconds between Slack DMs per twin (default 1800) |
| `COMPILE_INTERVAL_SECS` | background compile tick (default 1800; 0=off) |
| `NOTIFY_ON_COMPILE` | if true, HTTP compile may DM (default false; demo uses force) |

---

## 10. Preemptive Challenges & Mitigations

| Challenge | Mitigation |
|-----------|------------|
| Empty graph → empty product | Ensure V1→V2 path green; compiler returns honest empty Medium ledger |
| ACL regression | Only V2 APIs + TC-T06; no direct CRDB graph reads |
| Double publish | Unique ledger publish row + TC-T08 |
| Token exfil via prompt injection | No tokens in env; egress inject only (TC-T07) |
| Nag-bot perception | Veto-first, shadow mode, evidence refs |
| LLM hallucination | Optional LLM; structure-first; never invent items |
| Scope creep (Buzz/Centaur) | Explicit rejections §3.2; phase-2 parking lot |
| Out-of-order graph state | Rely on V2 temporal semantics; `graph_as_of` stamp on ledger |

---

## 11. Exhaustive Test Suite & Verification Matrix

| Test ID | Category | Scenario | Pass metric |
| :---- | :---- | :---- | :---- |
| **TC-T01** | Compile | Synthetic V2 fixtures → ledger items | Exact item set + evidence refs |
| **TC-T02** | Tier High | High + `high_auto_publish=true` → publish path | `publish_record` exists |
| **TC-T03** | Tier Medium | Medium → DM; silence after deadline → publish | One channel post; draft `published` |
| **TC-T04** | Veto | User vetoes → never channel post | No `publish_record`; status `vetoed` |
| **TC-T05** | Edit | Edit DM text → published body matches edit | `body_hash` matches `edited_text` |
| **TC-T06** | ACL | Compile cannot include private PR outside groups | 0 leaked private `node_id`s |
| **TC-T07** | Egress | Twin env has no Slack token; publish uses proxy | Secret scan empty; mock proxy called |
| **TC-T08** | Exactly-once | Double schedule publish same ledger | Single `publish_record` / single Slack ts |
| **TC-T09** | Shadow | Within shadow window: compile only, no DM | draft status `shadow`; 0 Slack calls |
| **TC-T10** | Sew E2E | V1 event → V2 project → V3 compile → draft | Full chain green in `sew_e2e.sh` |

---

## 12. Definition of Done

### 12.1 Vertical 3 MVP

- [ ] `status_twins` migrated on shared Cockroach cluster  
- [ ] twin-compiler produces ledgers from V2 ACL APIs  
- [ ] Confidence rollup deterministic + unit tested  
- [ ] Delivery state machine implements veto / edit / medium silence  
- [ ] Slack send only via egress `:18090`  
- [ ] TC-T01–TC-T10 pass (T10 may mock Slack)  
- [ ] ADR-013 recorded  
- [ ] README with ports, env, smoke  
- [ ] Metrics: `drafts_sent`, `veto_rate`, `publish_rate`, `compile_errors`, `acl_empty_rate`  

### 12.2 Product-Complete Sew Bar (platform)

- [ ] Live path: synthetic or real source event → V1 → V2 → V3 → (mock or real) Slack  
- [ ] Documented human demo script for CTO  
- [ ] Private graph still hidden after V1 group revoke (HybridMembership)  

Until §12.2 is green, do not claim full product completion.

---

## 13. Observability

| Metric | Meaning |
|--------|---------|
| `twin_compile_total{status}` | Compiles ok/error |
| `twin_drafts_sent_total` | DMs issued |
| `twin_veto_total` | Human vetoes |
| `twin_publish_total{result}` | Channel posts success/fail |
| `twin_acl_empty_total` | Compiles that saw zero ACL-visible graph |
| `twin_egress_fail_total` | Egress unavailable / denied |
| Lag gauges | Compile lag vs standup window |

Logs: structured JSON with `tenant_id`, `twin_id`, `ledger_id`, `draft_id` — never secrets.

---

## 14. Implementation Order (for the build session)

1. Confirm invariants in §1.2  
2. `twin-core` — types, confidence, state machine pure logic + unit tests  
3. `migrations/cockroach/001_init.sql`  
4. `twin-compiler` — V2 client + fixture mode  
5. `twin-delivery` — state transitions + mock egress Slack  
6. `twin-api` — Axum routes on `:18083`  
7. `twin-verify` — TC-T01–T10  
8. `scripts/smoke_v3.sh` + `scripts/sew_e2e.sh`  
9. Extend egress tool registry for Slack if needed  
10. README polish + metrics endpoints  

---

## 15. Document Control

| Field | Value |
|-------|--------|
| Ground truth for | Vertical 3 implementation |
| Supersedes | Informal plan chat outlines |
| Change process | ADR + edit this file; never silent scope expand |

*Document status: Specification for implementation. Implementation lives only under `ai-manager/vertical-3/`.*
