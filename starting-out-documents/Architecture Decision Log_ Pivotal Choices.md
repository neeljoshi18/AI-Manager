# Architecture Decision Log — Pivotal Choices

**Purpose:** Record key technology and product-architecture decisions for the AI Manager platform: options considered, trade-offs, what we chose, **why**, the **runner-up**, and when we should revisit.

**How to use:** Append new decisions as ADRs (Architecture Decision Records). Never delete old entries — mark them *Superseded* if replaced.

**Last updated:** 2026-07-24

---

## Index

| ID | Date | Decision | Choice | Status |
|----|------|----------|--------|--------|
| ADR-001 | 2026-07-21 | Vertical 1 ingestion runtime | Rust (Axum) | Active |
| ADR-002 | 2026-07-21 | Vertical 1 stream bus | Redpanda (Kafka API) | Active |
| ADR-003 | 2026-07-21 | Vertical 1 transactional / ACL store | CockroachDB | Active |
| ADR-004 | 2026-07-21 | Vertical 1 analytical store | ClickHouse | Active |
| ADR-005 | 2026-07-21 | Vertical 1 object vault | MinIO / S3 API | Active |
| ADR-006 | 2026-07-21 | Search / vectors / full-text index | **Strip** (do not build) | Active |
| ADR-007 | 2026-07-22 | Vertical 2 graph store | **CockroachDB property graph** (separate DB) | Active |
| ADR-008 | 2026-07-22 | Monorepo layout | Separate `vertical-N/` folders | Active |
| ADR-009 | 2026-07-22 | GitHub hosting | Private repo `AI-Manager` | Active |
| ADR-010 | 2026-07-22 | V2 live ACL source | Hybrid: read V1 `user_group_membership` | Active |
| ADR-011 | 2026-07-22 | Response to Buzz/Centaur | Context plane, not agent workspace/runtime | Active |
| ADR-012 | 2026-07-22 | Outbound secrets | Egress credential injection (Centaur pattern) | Active (MVP) |
| ADR-013 | 2026-07-22 | Vertical 3 product + stack | Status twins, veto-first Slack, Cockroach `status_twins` | Active (MVP implemented) |
| ADR-014 | 2026-07-22 | Notify vs ingest cadence | Continuous ingest; batched Slack notify | Active |
| ADR-015 | 2026-07-22 | Private Slack DMs | No silent 1:1 wiretap; bot/opt-in capture only | Active |
| ADR-016 | 2026-07-24 | Inference after learning window | **Hybrid Model Router** (rules → optional cloud → customer-prem local) | Active (planned) |

---

## ADR-001 — Vertical 1 ingestion runtime

| Field | Content |
|-------|---------|
| **Context** | Webhook edge needs flat tail latency under burst (CI storms). |
| **Options** | Go (Gin), Node (Fastify), Java (Quarkus), **Rust (Axum)** |
| **Choice** | **Rust + Axum + Tower** |
| **Runner-up** | Go (Gin) — faster to staff, still good latency |
| **Why** | No GC pauses; matches sub-50ms P99 ingestion invariant; memory safety for long-lived edge. |
| **Trade-offs** | Higher build complexity; slower iteration than Go/Node. |
| **Revisit if** | Team cannot maintain Rust and latency SLOs are easily met in Go benchmarks. |

---

## ADR-002 — Vertical 1 stream bus

| Field | Content |
|-------|---------|
| **Context** | Durable append-before-ACK; multi-consumer verticals later. |
| **Options** | Kafka, RabbitMQ, NATS, **Redpanda** |
| **Choice** | **Redpanda** (Kafka API) |
| **Runner-up** | Apache Kafka |
| **Why** | Kafka ecosystem without JVM ops pain; tiered storage story; V2+ can consume same topics. |
| **Trade-offs** | Enterprise features may cost; Kafka skill transfer is fine. |
| **Revisit if** | Managed Kafka is mandated by customer infra standards. |

---

## ADR-003 — Vertical 1 ACL / identity store

| Field | Content |
|-------|---------|
| **Context** | Real-time group membership + identity map with serializability. |
| **Options** | Postgres, MongoDB, DynamoDB, **CockroachDB** |
| **Choice** | **CockroachDB** |
| **Runner-up** | PostgreSQL (single-region) |
| **Why** | Distributed ACID for permission revocation; same store can host V2 graph DB later without a second transactional system. |
| **Trade-offs** | Slightly higher latency than single-node Postgres; ops model to learn. |
| **Revisit if** | CRDB ops cost dominates and we are forever single-region — then Postgres is enough. |

---

## ADR-004 — Vertical 1 analytical event store

| Field | Content |
|-------|---------|
| **Context** | Append-only events, sub-second filters/aggregations, dedup merges. |
| **Options** | Elasticsearch, Timescale, Snowflake, **ClickHouse** |
| **Choice** | **ClickHouse** (ReplacingMergeTree) |
| **Runner-up** | TimescaleDB |
| **Why** | Columnar compression + vectorized scans for telemetry volume; not used as a graph. |
| **Trade-offs** | Poor row updates (we only append). |
| **Revisit if** | Event volume stays tiny forever — Postgres would simplify. |

---

## ADR-005 — Object vault for raw payloads

| Field | Content |
|-------|---------|
| **Context** | Store raw webhook JSON without polluting OLAP/graph. |
| **Options** | Local FS only, GCS, **S3 API (MinIO local / S3 prod)** |
| **Choice** | **S3-compatible (MinIO in dev)** |
| **Runner-up** | Local filesystem only |
| **Why** | Production path matches cloud; DLQ + raw vault URIs on canonical events. |
| **Trade-offs** | Extra service in compose. |
| **Revisit if** | Never leave laptop-only demos. |

---

## ADR-006 — Full-text / vector / enterprise search

| Field | Content |
|-------|---------|
| **Context** | Glean competes on horizontal search; AI Manager competes on focus-time + graph reasoning. |
| **Options** | Build hybrid BM25+vectors, buy search, **strip entirely** |
| **Choice** | **Strip** — no BM25 corpus, no embedding index, no code search |
| **Runner-up** | Narrow metadata keyword search later (optional) |
| **Why** | ~90% TCO of Glean-class systems; not required for status orchestration moat. |
| **Trade-offs** | We will never win “search the whole company Drive.” That is intentional. |
| **Revisit if** | Enterprise buyers block purchase without a search SKU — then partner, don’t rebuild Glean. |

---

## ADR-007 — Vertical 2 Organizational Context Graph store

**Date:** 2026-07-22  
**Status:** Active  
**Deciders:** Product + engineering (session)  
**Related:** Competitive “relational reasoning wall”; V1 unblocks V2.

### Context

Vertical 2 must model **time**, **lineage**, and **entity relationships** over developer exhaust so agents do not get flat, out-of-order document chunks. It must:

- Consume V1 (not re-ingest GitHub).
- Enforce **zero-trust ACL** on every graph read (core product claim).
- Stay operable for **mid-market** engineering orgs (~25–100 developers) first.
- Prefer **systems that work and stay coherent** over premature scale optimization.

### Options deeply compared

#### Option A — CockroachDB property graph (nodes/edges tables, separate database)

| Pros | Cons |
|------|------|
| Same transactional system as V1 ACL already running | Multi-hop queries are recursive SQL, not Cypher |
| **Single-system ACL story:** identity, groups, graph edges, revocation can share serializable transactions / same ops | Not marketed as a “graph database” |
| No dual-write of permissions to a second engine | Need disciplined indexes and hop limits |
| Perfect isolation via separate DB `context_graph` without FKs into V1 | Extremely large enterprise graphs *might* outgrow recursive CTEs later |
| One backup, one HA story, one `sqlx` skillset | |
| Org-scale graphs (10³–10⁵ edges) are tiny for CRDB | |

#### Option B — Neo4j

| Pros | Cons |
|------|------|
| Excellent multi-hop Cypher UX | **Dual-write / dual-read ACL risk** — graph and CRDB identity can diverge under revocation races |
| Mature ecosystem | Heavier ops (JVM, licensing for enterprise features) |
| Matches “dedicated graph DB” language in competitive narrative | Mid-market cost and complexity for little gain at our graph size |
| | Second backup/HA plane; more ways for “works on my machine” to fail |
| | V1 already rejected sprawling infrastructure for TCO reasons |

#### Option C — Memgraph

| Pros | Cons |
|------|------|
| Cypher-like, high throughput streaming friendly | Still a **second source of truth** for graph vs ACL in CRDB |
| Lighter than classic Neo4j for some deployments | Newer ecosystem; another ops skill |
| Kafka/Redpanda consumption patterns exist | ACL must still be enforced carefully or dual-written |
| | Does not remove need for CRDB for V1 identity |

### Deeper discrepancy analysis (beyond “performance”)

| Concern | Cockroach graph | Neo4j / Memgraph |
|---------|-----------------|------------------|
| **ACL revocation &lt;200ms correctness** | Groups and edges in same transactional world; projector applies `events.acl` and graph filter uses same membership source | Easy to ship a bug where graph neighborhood returns a node the user’s groups no longer allow if cache/graph lags identity store |
| **Deploy ordering** | V1 + V2 share cluster; V2 uses new database only | Must orchestrate graph cluster + CRDB + bus; more failure modes |
| **Schema migration** | SQL migrations like V1 | Cypher/constraints + possibly different migration tooling |
| **Multi-tenant isolation** | Natural `tenant_id` leading keys | Must enforce in every query; easy to miss a path query |
| **“Fragile dependencies”** | One more schema on an already-chosen DB is **less** fragile than adding a whole graph product | New binary, ports, disk, monitoring, version pins |
| **Future V3 digital twins** | Twins call Graph API (not raw SQL); store can change later | Same if API is stable — but early dual systems slow V3 |
| **Scale path** | If multi-hop becomes hot, add Memgraph as **read projection** behind same Graph API | Starting here optimizes a problem we do not have yet |

### Decision

**Choose Option A — CockroachDB property graph in isolated database `context_graph`.**

### Why (explicit)

1. **Working systems first:** We already run and verified CRDB for V1. Extending it is the lowest integration risk.  
2. **ACL is a product feature, not a checkbox:** Dual-writing permissions into Neo4j/Memgraph creates the most dangerous class of bugs for this company.  
3. **Mid-market first:** Graph cardinality is modest; Cypher is a convenience, not a necessity.  
4. **Scale later:** Graph Query API stays stable; Memgraph/Neo4j can become a **projection** if path latency forces it (documented escape hatch).  
5. **User preference aligned:** Prefer CRDB unless a hard discrepancy appears — none outweighs ACL coherence at this stage.

### Runner-up

**Memgraph as future read projection** (not day-one SoT). Prefer Memgraph over Neo4j for lighter streaming if we ever project.

### What we will *not* do

- Put the context graph primarily in **ClickHouse** (wrong access pattern).  
- Use **Apache AGE on Cockroach** (compatibility risk).  
- Let V2 open its own GitHub webhooks (duplicates V1, splits ACL).

### Revisit triggers

Reopen ADR-007 if any of these become true:

- Multi-hop P95 path queries &gt; product SLO after indexing/tuning on CRDB.  
- Graph size grows past ~10M edges with heavy concurrent path queries.  
- Hire graph specialists and ops budget for a second data plane is intentional.  

Then evaluate **Memgraph projection** first; full SoT migration only with a written dual-run plan.

---

## ADR-008 — Monorepo vertical isolation

| Field | Content |
|-------|---------|
| **Context** | Multiple verticals; avoid one giant coupled workspace. |
| **Options** | Single Cargo workspace, polyrepo per vertical, **folder-per-vertical monorepo** |
| **Choice** | **`ai-manager/vertical-N/` separate trees** (separate Cargo workspaces) |
| **Runner-up** | Shared Cargo workspace with `crates/v1-*`, `crates/v2-*` |
| **Why** | Independent versioning/deploy; clear ownership; still one private Git repo. |
| **Trade-offs** | Some duplicated tooling; shared protos may need a later `contracts/` package. |
| **Revisit if** | Shared types duplication becomes painful — extract `contracts/`. |

---

## ADR-009 — GitHub private repository

| Field | Content |
|-------|---------|
| **Context** | Source of truth for collaboration and history. |
| **Options** | Public repo, private monorepo, multi-repo |
| **Choice** | **Private monorepo `AI-Manager`** |
| **Runner-up** | Private multi-repo per vertical |
| **Why** | Simple; keeps ground-truth docs + all verticals together; private for IP. |
| **Trade-offs** | Careful `.gitignore` so `target/` never lands in git. |
| **Revisit if** | Org policy requires separate repos per service. |

---

## Template for future ADRs

```markdown
## ADR-0XX — Title

| Field | Content |
|-------|---------|
| **Date** | YYYY-MM-DD |
| **Status** | Proposed / Active / Superseded by ADR-0YY |
| **Context** | … |
| **Options** | A, B, C |
| **Choice** | … |
| **Runner-up** | … |
| **Why** | … |
| **Trade-offs** | … |
| **Revisit if** | … |
```

---

## ADR-010 — Vertical 2 live ACL membership source

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active |
| **Context** | Private graph nodes carry allow-lists from event time; **who is in those groups** must match V1 after revocation or V2 leaks after V1-only revoke. |
| **Options** | (A) Local V2 membership only (B) Dual-write every revoke to V2 (C) **Read live groups from V1 Cockroach** |
| **Choice** | **C — HybridMembership**: `get_groups` prefers V1 `user_group_membership`; local table for demos/seeds |
| **Runner-up** | Dual-write revokes onto V2 membership |
| **Why** | Single source of truth for membership avoids critical dual-write lag; verified: V1 DELETE group → V2 private PR immediately 404 |
| **Trade-offs** | V2 production depends on V1 identity DB availability for authz (fail closed / local fallback only on read error) |
| **Revisit if** | Multi-region split of identity vs graph requires a dedicated membership cache with pub/sub |

---

## ADR-011 — Product stance vs Buzz (Block) and Centaur (Paradigm)

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active |
| **Context** | Buzz = multiplayer human+agent workspace (Nostr/Git/ACP). Centaur = secure Slack multiplayer agent runtime (K8s + iron-proxy). Risk of narrative collision with “AI manager / coworker.” |
| **Options** | (A) Compete as workspace (B) Compete as agent runtime (C) **Stay context/status plane** |
| **Choice** | **C** — AI Manager is the permissioned engineering context graph + meeting elimination layer |
| **Runner-up** | Partner/interop: expose graph via MCP to Buzz/Centaur/Goose |
| **Why** | Moat is relational reasoning + strip TCO + ACL; cloning Buzz/Centaur abandons strip-to-win and duplicates well-funded OSS |
| **Trade-offs** | We will not own multiplayer agent UX day one |
| **Revisit if** | Distribution requires shipping a thin agent shell; still use graph as core |

## ADR-012 — Outbound credential injection (Centaur-inspired)

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active (**MVP implemented** in `vertical-security/` + V1 `EgressClient`; required for V3 writes) |
| **Context** | Agents/tools with API keys in env are exfil-prone under prompt injection. Centaur iron-proxy injects secrets at egress; industry standard 2026. |
| **Options** | (A) Env vars forever (B) Full Centaur stack (C) **Scoped egress proxy + vault** |
| **Choice** | **C** — secrets vault + egress inject proxy + allowlist + audit/redact |
| **Runner-up** | Short-lived OAuth only (still needs careful agent isolation) |
| **Why** | Required for enterprise write path (V3); hardens V1 outbound; does not require multiplayer sandbox product |
| **Trade-offs** | Extra infra; must fail closed; doesn’t fix inbound webhook HMAC storage alone |
| **Verticals** | V3 critical; V1 medium (outbound); V2 low |
| **Revisit if** | Managed secret inject from cloud provider is mandated |

## ADR-013 — Vertical 3 Status Twins & Veto-First Delivery

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active (**MVP implemented** in `vertical-3/`; batch notify added) |
| **Context** | V1+V2 prove the context plane but do not yet kill standups. Product completeness requires status ledgers delivered privately first, then optionally published. Competitive pressure (Buzz workspace, Centaur agents, Geekbot forms) must not pull us off the context/status thesis. |
| **Options** | (A) Build multiplayer agent OS first (B) Build workspace chat first (C) **Status twins + ledger + veto-first Slack on top of V2** |
| **Choice** | **C** — Vertical 3 owns twins, `status_twins` Cockroach DB, deterministic confidence tiers, DM veto state machine, channel publish via egress only |
| **Runner-up** | Thin Slack bot with no ledger/versioning (too weak for enterprise audit + veto) |
| **Why** | Matches pitch (meeting elimination); reuses V2 ACL graph; reuses ADR-012 egress; avoids cloning Buzz/Centaur; structure-first reduces LLM hallucination risk |
| **Stack** | Rust/Axum; Cockroach `status_twins`; Redis timers; port **:18083**; Slack via egress **:18090** |
| **Non-goals** | Vector search; Centaur sandboxes; Buzz Nostr/Git; individual LOC rankings; god-mode graph SQL |
| **Ground truth** | `vertical-3/Technical Architecture Specification_ Vertical 3.md` |
| **Trade-offs** | Product value delayed until V3 ships; depends on V2 API quality and Slack bot setup |
| **Revisit if** | Mid-market buyers require Teams-first, or enterprise mandates a different workflow engine (Temporal) for multi-day sagas |
| **Tests** | TC-T01–TC-T10 in V3 TAS |

## ADR-014 — Continuous ingest, batched notify

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active |
| **Context** | GitHub “notify on every change” produces high webhook volume; mapping each event to a Slack DM floods developers and destroys trust. |
| **Options** | (A) DM per event (B) Drop most events (C) **Ingest all events; schedule status DMs** |
| **Choice** | **C** — V1/V2 ingest continuous; V3 `NOTIFY_INTERVAL_SECS` / `STATUS_WINDOW_SECS` / scheduler; bridge does not Slack |
| **Why** | Company brain needs full exhaust; humans need digests + outcomes (merged/closed), not keystroke theater |
| **Revisit if** | Real-time paging for sevs is required (separate alert path, not standup twin) |

## ADR-015 — Private Slack DMs and intent capture

| Field | Content |
|-------|---------|
| **Date** | 2026-07-22 |
| **Status** | Active |
| **Context** | Founder wants colleague 1:1 DMs as intent signal for digital twins; Slack platform and ethics constrain silent read. |
| **Options** | (A) Enterprise compliance scrape all DMs (B) Silent bot wiretap (C) **Bot-mediated + opt-in capture only** |
| **Choice** | **C** — No silent human↔human DM read by default; capture via bot DM, slash capture, channels where bot is invited; compliance SKU only with legal |
| **Why** | Trust is the product; mid-market can’t buy spyware; technical bot membership model |
| **Revisit if** | Enterprise customer contracts compliance API with dual-control policy |

## ADR-016 — Customer-prem model inference after learning window

| Field | Content |
|-------|---------|
| **Date** | 2026-07-24 |
| **Status** | Active (**planned** — docs first; code after M6 multi-member beta path) |
| **Context** | Founder vision: 10–14 day shadow/learning window with continuous ingest; after the company is a live client, prefer **local SLM/LLM on customer sandbox** so agents do not forever pay cloud token tax and inference stays in the customer boundary. Must not become Glean (no proprietary corpus hosting) or Centaur (not a multiplayer agent OS). |
| **Options** | (A) Always cloud paid APIs for all prose/intent (B) Local model day-one before product works (C) **Hybrid Model Router: rules default → optional cloud during shadow → customer-prem local after gold pairs exist** |
| **Choice** | **C — Hybrid Model Router** |
| **Runner-up** | Always cloud for SMB SKU only |
| **Why** | Continuous **ingest** is the company brain (ADR-014); continuous **paid inference** is not required. Structure-first digests already work rules-only. Shadow produces approved text/intent labels for distill/LoRA without hosting source. Customer-prem inference matches privacy pitch and unit economics at mid-market volume. |
| **What we train on** | Structured ledgers, human edit/veto gold text, intent labels — **not** raw git blobs or Drive dumps (ADR-006). |
| **What agents do** | Watch graph + draft via router; humans veto (ADR-011). Local model does not hold Slack/GitHub long-lived tokens (ADR-012 egress for tools). |
| **Sequence** | M6 multi-member + thin agents + intent/conflict v0 → design-partner shadow → **then** Model Router + local serve (Ollama/vLLM recipe). Do not train before multi-person product. |
| **Trade-offs** | On-prem GPU/CPU ops burden for some customers; small tenants may stay rules/cloud; training quality depends on shadow participation. |
| **Ground truth** | `plans/2026-07-24_onprem-model-and-agents.md`; Product Roadmap § Learning window |
| **Revisit if** | Open models fail rewrite quality gates; enterprise mandates specific hosted model; or mid-market refuses any GPU and cloud cost is still acceptable |

## Changelog

| Date | Change |
|------|--------|
| 2026-07-22 | Created log; ADR-007 Cockroach for V2 graph; monorepo + GitHub ADRs. |
| 2026-07-22 | Implemented V2; **ADR-010** live ACL from V1 identity tables. |
| 2026-07-22 | Buzz/Centaur research; **ADR-011** product stance; **ADR-012** egress secrets planned; companion docs in starting-out-documents. |
| 2026-07-22 | **ADR-012** MVP in `vertical-security/`; **ADR-013** V3 status twins TAS + session handoff for new chat. |
| 2026-07-22 | V3 MVP + sew + GitHub/Slack live; **ADR-014** batch notify; **ADR-015** private DM policy; product roadmap file. |
| 2026-07-24 | **ADR-016** hybrid Model Router / customer-prem inference after learning window; plan `2026-07-24_onprem-model-and-agents.md`. |
