# Plan of Action — Vertical 2 (Organizational Context Graph)

**Date:** 2026-07-22  
**Status:** V2 core implemented (embedded + CRDB); this plan covers **next** work in light of Buzz/Centaur landscape.  
**Owner:** Engineering  

---

## 1. North star (unchanged)

Vertical 2 is the **ACL-safe temporal context graph** over developer exhaust. It unblocks:

- Vertical 3 digital twins / status ledgers  
- Optional “graph as context backend” for external agents (Buzz, Centaur, Goose, Cursor)

It is **not** a multiplayer chat product and **not** a secure agent sandbox product.

---

## 2. Current state (done)

| Item | Status |
|------|--------|
| Spec | `vertical-2/Technical Architecture Specification_ Vertical 2.md` |
| Implementation | `graph-core`, `graph-api`, `graph-projector`, `graph-verify` |
| TC-G01…G10 | Passing (embedded) |
| Production graph DB | Cockroach `context_graph` |
| Live ACL hybrid | Reads V1 `user_group_membership` when `V1_COCKROACH_URL` set |
| GitHub | Private `AI-Manager` monorepo |

---

## 3. Phased plan

### Phase V2-A — Production coupling (near term)

**Goal:** Graph stays fresh without manual `POST /v2/project`.

| Work | Notes |
|------|--------|
| Harden `graph-projector` | Committed offsets; consume `events.raw` + `events.acl`; restart-safe |
| Expand mappers | Jira/Linear/Slack identity → richer edges (ASSIGNED_TO, MEMBER_OF, DISCUSSED_IN) |
| Backfill job | Replay from V1 ClickHouse or bus `events.backfill` |
| Metrics | Projector lag, apply rate, ACL deny, hop truncations |
| Tests | TC-G08/G09 against real Redpanda; integration script V1 ingest → auto project |

**Exit:** New GitHub PR webhook → appears in graph within lag SLO without HTTP bridge.

### Phase V2-B — Query quality for agents

**Goal:** Vertical 3 can trust graph answers.

| Work | Notes |
|------|--------|
| Path/neighborhood SLOs | Index tuning; max-hop policy |
| Blockers / critical path APIs | Productized for standup replacement |
| Tenant isolation audit | Fuzz cross-tenant IDs |
| Docs | OpenAPI for graph-api |

**Exit:** Documented API contract frozen for V3.

### Phase V2-C — Coopetition interface (optional)

**Goal:** External agent runtimes use us as memory.

| Work | Notes |
|------|--------|
| MCP or REST “context pack” | ACL-filtered neighborhood/state for a user+resource |
| No secret inject required | Read-only |
| Compatible with Centaur tools / Buzz agents | They execute; we contextualize |

**Exit:** One external harness demo (e.g. Goose or curl agent) answers “what’s blocking this PR?” from our graph only.

### Explicit non-goals for V2

- Building Buzz (Nostr workspace, Git forge)  
- Building Centaur (K8s multiplayer Slack agent kernel)  
- Full-text / vector search  

---

## 4. Dependency on security work

If/when **egress credential proxy** ships (see companion plan):

- V2 **read path** largely unchanged.  
- Any V2 worker that calls GitHub/Slack APIs for enrichment must use the proxy.  
- V2 does **not** block on proxy for core graph correctness.

---

## 5. Success metrics

| Metric | Target (initial) |
|--------|------------------|
| Projection lag P95 | &lt; 30s after V1 accept |
| ACL leak rate | 0 in automated suites |
| Multi-hop path for PR→repo | &lt; 100ms embedded; &lt; 500ms CRDB mid-market |
| Manual bridge required | No (after V2-A) |

---

## 6. Suggested order for coding agent

1. Projector offset commits + dual-topic consume  
2. Integration test: V1 production bus → V2 graph  
3. Mapper coverage  
4. Graph API OpenAPI + smoke in CI  
5. (Later) MCP context pack  

---

*Update this file when a phase exits or is deprioritized.*
