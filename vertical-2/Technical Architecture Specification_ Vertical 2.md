# Technical Architecture Specification: Vertical 2  
## Organizational Context Graph — Temporal Lineage, Dependencies & ACL-Safe Multi-Hop Reasoning

---

## 1. Executive Summary & Core Invariants

### 1.1 Scope & Purpose

**Vertical 2** is the relational reasoning layer of the Autonomous AI Manager platform. It consumes Vertical 1 telemetry (never re-ingests source systems), projects developer exhaust into a specialized **Organizational Context Graph**, and exposes ACL-filtered multi-hop queries for Vertical 3+ (digital twins, status ledgers, agent negotiation).

Vertical 2 exists to defeat the **relational reasoning wall**: search engines return flat chunks; agents need **temporal state**, **explicit lineage**, and **entity relationships**.

Vertical 2 does **not** host proprietary source code, full document bodies, or vector indices.

### 1.2 System Invariants (Non-Negotiable Guarantees)

1. **Consumer-Only Ingestion:** Vertical 2 never accepts GitHub/GitLab/Jira/Slack webhooks. The sole write path is projection from Vertical 1 durable streams (and controlled backfill via V1 APIs).
2. **Zero-Trust Graph Isolation:** No graph read may return a node or edge unless the request `QueryContext` satisfies ACL constraints denormalized from Vertical 1 (same semantics as V1: public OR `hasAny(user_groups, allowed_group_ids)`).
3. **Temporal Correctness:** Entity state and path answers are reconstructed using **origin event timestamps** (`valid_from` / event time), not projector wall-clock order. Out-of-order events must not corrupt “current state.”
4. **Metadata-Only Graph:** Nodes and edges store collaborative metadata and foreign resource IDs only — never file patches, full message bodies, or embeddings.
5. **No Upward Coupling:** Vertical 1 must not depend on Vertical 2. Vertical 2 may depend on V1 contracts (topics, event shape, identity tables read-only).

### 1.3 Relationship to Vertical 1

| Concern | Owner |
|---------|--------|
| Webhooks, HMAC, dedup, raw vault | Vertical 1 |
| Canonical event log (ClickHouse) | Vertical 1 |
| Identity map + live group membership | Vertical 1 (Cockroach) |
| Stream bus topics | Vertical 1 (Redpanda) |
| Graph projection + multi-hop API | **Vertical 2** |
| Digital twins / Slack veto UX | Vertical 3+ (future) |

---

## 2. Technical Stack Selection & Trade-Off Matrix

### 2.1 Pipeline Diagram

```
+-----------------------------------------------------------------------------------+
| VERTICAL 1 REDPANDA                                                               |
| topics: events.raw | events.realtime | events.acl | events.backfill               |
+-----------------------------------------------------------------------------------+
                                        |
                                        v
+-----------------------------------------------------------------------------------+
| GRAPH PROJECTOR (RUST)                                                            |
| [ Offset commit ] --> [ Event→Node/Edge mappers ] --> [ Temporal upsert txn ]     |
| [ ACL fields denormalized onto nodes/edges from event.acl_context ]               |
+-----------------------------------------------------------------------------------+
                                        |
                                        v
+-----------------------------------------------------------------------------------+
| COCKROACHDB  database: context_graph                                              |
| [ graph_node ] [ graph_edge ] [ entity_state ] [ projector_offsets ]              |
+-----------------------------------------------------------------------------------+
                                        |
                                        v
+-----------------------------------------------------------------------------------+
| GRAPH API (RUST / AXUM)                                                           |
| [ Resolve groups ] --> [ ACL-safe SQL / recursive CTE ] --> [ Path / State DTO ]  |
+-----------------------------------------------------------------------------------+
                                        |
                                        v
                          Vertical 3+ / internal agents
```

### 2.2 Stack Comparison & Justification

| Architectural Layer | Evaluated Alternatives | Selected Architecture | Technical Rationale & Trade-Off Analysis |
| :--- | :--- | :--- | :--- |
| **Graph source of truth** | Neo4j, Memgraph, ClickHouse-as-graph, Apache AGE | **CockroachDB property-graph tables** in isolated DB `context_graph` | **Decision:** Reuses the V1 transactional plane already proven for ACL; avoids dual-write permission races; mid-market graph sizes fit recursive CTEs. **Trade-Off:** Multi-hop ergonomics weaker than Cypher; mitigated by a stable Graph API and optional future Memgraph **projection** (not day-one SoT). See ADR-007. |
| **Projector / API runtime** | Go, Node, Python | **Rust (Axum)** | Same as V1; shared operational culture; safe concurrent consumers. |
| **Event input** | Poll V1 HTTP only, dual webhooks | **Redpanda consumer** (V1 topics) | Zero data loss path already guaranteed by V1; projector is restart-safe via offsets. |
| **Identity / groups** | Copy into graph-only store | **Read V1 membership + denormalize ACL on edges** | Live revocation from V1; historical edge ACL from event snapshot at write time. |
| **Hot path cache** | None, full Neo4j | **Optional Redis** (existing V1 Redis) | Phase-2 ego-network cache only; not required for correctness. |
| **Search / vectors** | OpenSearch, pgvector | **None** | Strategic strip vs Glean (ADR-006). |
| **Serialization** | Ad-hoc JSON only | **HTTP JSON + optional Protobuf** for stable DTOs | Aligns with V1 determinism for cross-vertical contracts. |

### 2.3 Explicit rejections

| Rejected | Why |
|----------|-----|
| ClickHouse as primary graph | Analytical scans ≠ multi-hop lineage walks |
| Neo4j as day-one SoT | Dual-write ACL risk; mid-market weight; second HA plane |
| Memgraph as day-one SoT | Same second-system risk; better as future projection |
| Apache AGE on CRDB | Fragile Postgres-extension assumptions on Cockroach |
| V2 webhook receivers | Splits ingestion + ACL ownership from V1 |

---

## 3. Subsystem Architecture & Pipeline Specification

### 3.1 Graph Projector

1. **Subscribe** to Vertical 1 topics (minimum: `events.raw`, `events.acl`; optionally `events.realtime` / `events.backfill` with separate consumer groups).
2. **Deserialize** Vertical 1 bus payload (`CanonicalEvent` / JSON envelope used by V1).
3. **Map** event_type → graph mutations (idempotent upsert by natural keys).
4. **Transactionally** write nodes, edges, and `entity_state` in `context_graph`.
5. **Commit offsets** only after successful transaction (at-least-once; handlers must be idempotent).

**Idempotency key:** `(tenant_id, event_id)` recorded in `projector_applied_events` or equivalent unique constraint on edge/event linkage.

### 3.2 Canonical Graph Schema (CockroachDB DDL sketch)

```sql
CREATE DATABASE IF NOT EXISTS context_graph;

-- Logical graph node
CREATE TABLE IF NOT EXISTS context_graph.graph_node (
    tenant_id        STRING NOT NULL,
    node_id          STRING NOT NULL,  -- stable: e.g. person:gu_…, pr:acme/app/pr/7
    node_type        STRING NOT NULL,  -- Person|Repo|PullRequest|Issue|Ticket|Team|Channel|Commit
    display_name     STRING NOT NULL DEFAULT '',
    resource_id      STRING NOT NULL DEFAULT '',  -- V1 resource_id when applicable
    properties_json  JSONB NOT NULL DEFAULT '{}'::JSONB,
    is_private       BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    acl_version      INT8 NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_node_type ON context_graph.graph_node (tenant_id, node_type);
CREATE INDEX IF NOT EXISTS idx_node_resource ON context_graph.graph_node (tenant_id, resource_id);

-- Directed edges with temporal validity
CREATE TABLE IF NOT EXISTS context_graph.graph_edge (
    tenant_id         STRING NOT NULL,
    edge_id           STRING NOT NULL,  -- deterministic hash or UUID
    edge_type         STRING NOT NULL,  -- AUTHORED|BLOCKS|SUPERSEDES|…
    from_node_id      STRING NOT NULL,
    to_node_id        STRING NOT NULL,
    valid_from        TIMESTAMPTZ NOT NULL,
    valid_to          TIMESTAMPTZ NULL,   -- NULL = currently open
    event_id          STRING NOT NULL,    -- originating V1 event
    properties_json   JSONB NOT NULL DEFAULT '{}'::JSONB,
    is_private        BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    acl_version       INT8 NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, edge_id)
);

CREATE INDEX IF NOT EXISTS idx_edge_from ON context_graph.graph_edge (tenant_id, from_node_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edge_to ON context_graph.graph_edge (tenant_id, to_node_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edge_active ON context_graph.graph_edge (tenant_id, edge_type, valid_to);

-- Current/derived state per entity (PR open/closed, ticket status, …)
CREATE TABLE IF NOT EXISTS context_graph.entity_state (
    tenant_id        STRING NOT NULL,
    node_id          STRING NOT NULL,
    state_key        STRING NOT NULL,  -- e.g. lifecycle, status
    state_value      STRING NOT NULL,  -- OPEN|CLOSED|…
    as_of            TIMESTAMPTZ NOT NULL,
    event_id         STRING NOT NULL,
    is_private       BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    PRIMARY KEY (tenant_id, node_id, state_key)
);

CREATE TABLE IF NOT EXISTS context_graph.projector_offsets (
    consumer_group STRING NOT NULL,
    topic          STRING NOT NULL,
    partition_id   INT8 NOT NULL,
    next_offset    INT8 NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (consumer_group, topic, partition_id)
);

CREATE TABLE IF NOT EXISTS context_graph.projector_applied_events (
    tenant_id  STRING NOT NULL,
    event_id   STRING NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, event_id)
);
```

### 3.3 Node & Edge Vocabulary (v1)

**Node types:** `Person`, `Team`, `Repo`, `PullRequest`, `Issue`, `Ticket`, `Channel`, `Commit` (SHA metadata only).

**Edge types:**

| Edge | Meaning | Typical source events |
|------|---------|------------------------|
| `AUTHORED` | Person → PR/Commit/Issue | PR opened, push |
| `REVIEWED` | Person → PR | PR review |
| `MERGED_INTO` | PR → Repo/branch node | PR merged |
| `BELONGS_TO` | PR/Issue → Repo | any code/work item |
| `IMPLEMENTS` | PR → Ticket/Issue | branch name / linked issue metadata |
| `BLOCKS` / `BLOCKED_BY` | Issue/PR ↔ Issue/PR | labels, links, explicit blocker fields |
| `SUPERSEDES` | Ticket/Issue → Ticket/Issue | identity lineage when detectable |
| `ASSIGNED_TO` | Ticket → Person | issue assigned |
| `MEMBER_OF` | Person → Team | identity events |
| `DISCUSSED_IN` | Work item → Channel | Slack refs (metadata only) |

### 3.4 Mapping Rules (illustrative)

**`pull_request.opened`**

- Upsert nodes: `Repo(resource=parent)`, `PullRequest(resource_id)`, `Person(actor)`.
- Edges: `AUTHORED` (Person→PR), `BELONGS_TO` (PR→Repo).
- State: `lifecycle=OPEN` at `event_timestamp`.
- Copy `is_private`, `allowed_group_ids`, `acl_version` from event ACL snapshot.

**`pull_request.closed` / merged**

- State: `lifecycle=CLOSED` or `MERGED` with `as_of = event_timestamp` (wins over earlier OPEN if timestamp later).
- Do not delete historical edges; close temporal validity where appropriate.

**ACL revocation (`events.acl` / identity remove)**

- Projector updates **live** membership via V1 identity store for query-time filters.
- Does **not** rewrite history of private edges; query filter uses **current groups** against edge/node allow-lists (same as V1 CH query pattern). Optionally bump a tenant graph generation for cache bust.

### 3.5 Graph API (ACL-safe)

All endpoints require `tenant_id` + `user_id` (global_user_id).

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/v2/tenants/{tenant}/nodes/{node_id}` | Fetch node if ACL allows |
| `GET` | `/v2/tenants/{tenant}/nodes/{node_id}/neighborhood` | 1..N hop neighbors (bounded) |
| `GET` | `/v2/tenants/{tenant}/path` | Shortest/bounded path between two nodes |
| `GET` | `/v2/tenants/{tenant}/state` | Entity state as-of timestamp |
| `GET` | `/v2/tenants/{tenant}/blockers` | Open BLOCKS edges for a person/team |
| `GET` | `/healthz` `/readyz` `/metrics` | Ops |

**Mandatory filter fragment (conceptual SQL):**

```sql
AND (n.is_private = false OR n.allowed_group_ids && $user_groups)
AND (e.is_private = false OR e.allowed_group_ids && $user_groups)
```

Hop depth default **3**, hard max **6** to bound CTE cost.

### 3.6 Query-Time Group Resolution

1. Load `user_groups` from Vertical 1 Cockroach `user_group_membership` (read-only) **or** V2-local cache invalidated on `events.acl`.
2. Prefer live groups for authorization (revocation correctness).
3. Never return edges the user cannot see even if path intermediate would require them — either omit path or return `403/empty` consistently (spec: **fail closed** → empty neighborhood, never leak node labels on denied private nodes).

---

## 4. Preemptive Architectural Challenges & Mitigations

| Challenge | Mitigation |
|-----------|------------|
| Out-of-order webhooks | State by `event_timestamp`; temporal edges ordered by `valid_from` |
| At-least-once bus delivery | `projector_applied_events` unique on `(tenant_id, event_id)` |
| ACL revocation races | Live group membership at query; integration test TC-G03 |
| Mapper schema drift (new event types) | Unknown types → metrics + skip; no crash |
| Backfill storms | Dedicated consumer group on `events.backfill`; rate-limit projector |
| Recursive CTE cost | Max hops; indexes on from/to; optional Redis neighborhood cache later |
| Cross-DB consistency | No distributed FK; eventual projection lag measured as consumer lag |

---

## 5. Integration Contract with Vertical 1

### 5.1 Topics

| Topic | V2 use |
|-------|--------|
| `events.raw` | Primary projection feed |
| `events.realtime` | Optional low-latency consumer group |
| `events.backfill` | Historical rebuild |
| `events.acl` | Membership change / cache invalidation |

### 5.2 Required event fields for projection

From V1 canonical record:

- `event_id`, `tenant_id`, `provider`, `category`, `event_type`
- `event_timestamp`, `resource_id`, `parent_resource_id`
- `actor.global_user_id`, `actor.provider_user_id`
- `acl.is_private`, `acl.allowed_group_ids`, `acl.acl_version`
- `attributes` (typed subset per mapper; ignore unknown keys)

### 5.3 Forbidden couplings

- V2 must not `INSERT/UPDATE` V1 tables `canonical_events_*` or `tenants` (except read).
- V1 must not call V2 at request path for webhook ACK.

---

## 6. Repository & Runtime Layout

```
ai-manager/vertical-2/
├── Technical Architecture Specification_ Vertical 2.md  # this document
├── README.md
├── Cargo.toml                    # workspace (implementation phase)
├── crates/
│   ├── graph-core/               # domain, SQL, ACL helpers
│   ├── graph-projector/          # Redpanda → CRDB
│   ├── graph-api/                # Axum query surface
│   └── graph-verify/             # TC-G01… battery
├── migrations/cockroach/
│   └── 001_init.sql
├── proto/                        # optional graph DTOs
└── scripts/
```

**Runtime env (shares V1 infra):**

- `KAFKA_BROKERS` (V1 Redpanda)
- `COCKROACH_URL` (same cluster; V2 opens DB `context_graph`)
- `REDIS_URL` (optional)
- `V1_IDENTITY_DATABASE` / search_path for read-only membership

---

## 7. Exhaustive Test Suite & Verification Matrix

| Test ID | Category | Scenario | Pass metric |
| :---- | :---- | :---- | :---- |
| **TC-G01** | Projection | PR opened event → nodes Repo/PR/Person + AUTHORED + BELONGS_TO | Exact edge set present |
| **TC-G02** | Temporal | CLOSED then OPENED out-of-order → lifecycle CLOSED | Matches V1 TC-06 semantics |
| **TC-G03** | ACL | Private PR neighborhood as unauthorized user | 0 nodes leaked |
| **TC-G04** | ACL revoke | Remove group; re-query private PR | Immediate empty (live groups) |
| **TC-G05** | Idempotency | Replay same `event_id` 1000× | Single applied row; no duplicate edges |
| **TC-G06** | Multi-hop | Person→PR→Repo path length 2 | Path returned; max-hop respected |
| **TC-G07** | Blockers | BLOCKS edge open → `/blockers` lists it | Only ACL-visible blockers |
| **TC-G08** | Chaos | Restart projector mid-batch | At-least-once; final graph correct |
| **TC-G09** | Backfill | Load historical `events.backfill` | Lag drains; states coherent |
| **TC-G10** | Isolation | Tenant A events never visible to tenant B user | 0 cross-tenant rows |

---

## 8. Definition of Done (Exit Criteria for Vertical 2)

Vertical 2 unblocks Vertical 3 when:

- [ ] `context_graph` schema migrated on shared Cockroach cluster  
- [ ] Projector consumes `events.raw` + `events.acl` with committed offsets  
- [ ] Graph API enforces ACL on all read paths  
- [ ] TC-G01–TC-G10 pass in embedded/integration mode  
- [ ] Documented contract with V1 topics and field requirements  
- [ ] ADR-007 recorded in Architecture Decision Log  
- [ ] No Neo4j/Memgraph required for correctness (projection optional only)  
- [ ] Observability: projector lag, apply rate, ACL deny count, query P95  

---

## 9. Observability Baseline

| Metric | Description |
|--------|-------------|
| `v2_projector_events_applied_total` | Successful projections |
| `v2_projector_events_skipped_total` | Unknown types / duplicates |
| `v2_projector_lag` | Bus lag by topic/partition |
| `v2_graph_query_ms` | P50/P95/P99 |
| `v2_acl_deny_total` | Filtered empty results vs hard errors |
| `v2_cte_hop_ truncated_total` | Hit max hop depth |

---

## 10. Future Escape Hatch (Not In Scope for V2 v1)

If multi-hop latency exceeds SLOs after indexing:

1. Keep **Graph API** unchanged.  
2. Add **Memgraph** (runner-up) as a **read projection** filled by the same projector dual-write or CDC.  
3. Do **not** move ACL source of truth off Cockroach/V1 identity.  

Document any such change as a new ADR superseding only the *storage projection*, not ACL ownership.

---

## 11. References

- `starting-out-documents/Technical Architecture Specification_ Vertical 1.md`  
- `starting-out-documents/Deep Dive_ Glean Competitive Analysis.md` (relational reasoning wall; metadata-only graph)  
- `starting-out-documents/Technical Audit and Competitive Strategy Report_ …md`  
- `starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` (**ADR-007**)  
- Vertical 1 implementation: `vertical-1/`  

---

*Document status: Specification for implementation. Implementation lives only under `ai-manager/vertical-2/`.*
