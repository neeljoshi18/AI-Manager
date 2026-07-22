# Session Handoff — AI Manager State

**Date:** 2026-07-22  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager`  
**Purpose:** Grounded state pack for a **new chat** (fresh context window). Prefer this file + vertical specs over auto-compacted chat history.

---

## 1. Product thesis (non-negotiable)

| Stance | Detail |
|--------|--------|
| **What we are** | Permissioned **engineering context plane** + **meeting elimination** (status ledgers) |
| **Anti-Glean** | No vector search, no full-text enterprise index, no proprietary source code hosting, no OCR document corpus |
| **Not Buzz** | Not a multiplayer human+agent workspace / Nostr / Git forge |
| **Not Centaur** | Not a K8s multiplayer agent OS; **steal only** egress credential injection |
| **Success metric** | Meetings deleted / focus time reclaimed — not search engagement |

**Competitive docs:**  
- `starting-out-documents/Competitive Landscape Update_ Buzz and Centaur.md`  
- Glean analysis docs in `starting-out-documents/`  
- ADRs 006, 011, 012, **013**

---

## 2. Monorepo layout

| Path | Role | Status |
|------|------|--------|
| `starting-out-documents/` | Strategy, TAS (V1), decision log, competitive, this handoff | Living |
| `vertical-1/` | Telemetry ingest, ACL, bus, CH, CRDB, MinIO | ~85–90% done |
| `vertical-2/` | Organizational context graph + projector + API | ~75–80% done |
| `vertical-security/` | Centaur-inspired egress credential proxy | MVP done |
| `vertical-3/` | Status twins, ledgers, veto-first Slack delivery | **Spec only** (implement next) |

**Product “100%” bar:** V1 + V2 + Security + V3 **sewn** on one E2E path (not “V1 green alone”).

```
Source webhook → V1 accept → V2 graph → V3 ledger → Slack DM (egress) → veto/edit/silence → channel post
```

---

## 3. Ports & services

| Service | Port | Notes |
|---------|------|-------|
| V1 ingestion / query | **18080** | Avoid host 8080 (often Ollama) |
| V2 graph-api | **18082** | |
| V3 twin-api | **18083** | Spec; not implemented yet |
| Egress proxy | **18090** | `vertical-security` |
| Redis (compose) | 6379 | Dedup, rate limit, future veto timers |
| Redpanda Kafka API | **19092** | (mapped; not 9092 alone) |
| Cockroach SQL | 26257 | DBs: `defaultdb` (V1), `context_graph` (V2), `status_twins` (V3 planned) |
| ClickHouse HTTP | 8123 | V1 analytics |
| MinIO | 9000 | Raw webhook vault |

---

## 4. What works (verified patterns)

### Vertical 1

- Rust/Axum workspace: `telemetry-core`, `ingestion`, `consumer`, `query`, `verify`, `proto`
- HMAC, Redis dedup/rate limit, Redpanda bus, Cockroach ACL/identity, ClickHouse ReplacingMergeTree-style, MinIO vault
- Production wiring under `telemetry-core/production/`
- `SKIP_AUTH=true` for local; production needs secrets
- Egress client module in core; smoke: `scripts/egress_smoke.sh`
- Verify battery: `cargo run -p telemetry-verify` (TC-01…)
- Docker: `vertical-1/docker-compose.yml`

### Vertical 2

- Workspace: `graph-core`, `graph-api`, `graph-projector`, `graph-verify`
- Cockroach DB `context_graph`; temporal nodes/edges; ACL on nodes/edges
- **HybridMembership (ADR-010):** live groups from V1 `user_group_membership` — V1 revoke hides private graph nodes immediately
- HTTP project bridge: `POST /v2/project` + `scripts/integration_v1_bridge.sh`
- Bus projector: topics `events.raw`, `events.acl`; production offsets in CRDB
- Verify: `cargo run -p graph-verify` (TC-G01–G10)
- Spec: `vertical-2/Technical Architecture Specification_ Vertical 2.md`

### Vertical Security

- Fail-closed egress proxy; tool registry YAML; file secrets backend (dev)
- Inject Authorization; audit log (no secret values); optional response redact
- `cargo test` in `vertical-security/`; run on `:18090`
- Do **not** put long-lived tokens in twin/worker env (ADR-012)

### Vertical 3

- **Ground truth only:** `vertical-3/Technical Architecture Specification_ Vertical 3.md`
- No Rust crates yet — next session implements per TAS
- Crates planned: `twin-core`, `twin-compiler`, `twin-delivery`, `twin-api`, `twin-verify`
- Invariants: veto-first, confidence tiers, ACL via V2 only, Slack via egress only

---

## 5. Commands that should be green

```bash
# V1
cd vertical-1 && cargo test && cargo run -p telemetry-verify

# V2
cd vertical-2 && cargo test && cargo run -p graph-verify
./scripts/integration_v1_bridge.sh   # needs V2 up; V1 optional

# Security
cd vertical-security && cargo test

# Infra (optional)
cd vertical-1 && docker compose up -d
```

**Production V2 sketch** (V1 compose up):

```bash
docker compose -f vertical-1/docker-compose.yml exec -T cockroach \
  ./cockroach sql --insecure -e "CREATE DATABASE IF NOT EXISTS context_graph;"
# apply vertical-2/migrations/cockroach/001_init.sql
RUNTIME_MODE=production \
  COCKROACH_URL='postgresql://root@127.0.0.1:26257/context_graph?sslmode=disable' \
  V1_COCKROACH_URL='postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable' \
  cargo run -p graph-api   # from vertical-2
```

---

## 6. Open risks / known gaps

| Risk | Notes |
|------|-------|
| V2 projector multi-partition / poison offsets | Production offset commit exists; chaos edge cases remain |
| Vault backends | Dev file secrets only; enterprise vault still future |
| ADR-012 index row still said “planned” historically | MVP implemented in `vertical-security/` — treat as Active/MVP |
| V3 not built | Spec frozen; product value blocked until twins ship |
| Empty graph → empty ledgers | Prefer live V1→V2 path before polishing twin UX |
| Context auto-compact | Lossy; use this handoff + specs, not chat alone |

---

## 7. Architecture decisions to obey

| ADR | Choice |
|-----|--------|
| 006 | Strip search/vectors |
| 007 | V2 graph on Cockroach `context_graph` |
| 008 | Separate `vertical-N/` folders |
| 010 | HybridMembership from V1 |
| 011 | Context plane, not Buzz/Centaur clone |
| 012 | Egress credential injection |
| **013** | V3 status twins + veto-first delivery (see decision log) |

---

## 8. Frozen for Vertical 3 implementation

Read **`vertical-3/Technical Architecture Specification_ Vertical 3.md`** end-to-end before coding.

Highlights:

- Port **:18083**
- DB **`status_twins`**
- Compiler reads **V2 ACL APIs only** (no god-mode SQL on `context_graph`)
- Slack **only** via egress `:18090` tool `slack_api`
- Confidence: High / Medium / Blocker (deterministic first)
- Shadow mode default 10 days; `high_auto_publish=false` initially
- Tests TC-T01–TC-T10; scripts `smoke_v3.sh`, `sew_e2e.sh`
- Do not build sandboxes, Nostr workspace, or surveillance rankings

---

## 9. Suggested new-chat implementation order

1. Confirm TAS invariants aloud  
2. `twin-core` domain (ledger, confidence, draft state machine)  
3. `migrations/cockroach/001_init.sql`  
4. `twin-compiler` (V2 HTTP client + fixtures)  
5. `twin-delivery` (veto machine; mock Slack via egress)  
6. `twin-api` on :18083  
7. `twin-verify` TC-T01–T10  
8. `scripts/sew_e2e.sh`  
9. Extend `vertical-security` tool registry for Slack if missing  
10. README + metrics  

---

## 10. Human demo path (target after V3 MVP)

1. `docker compose` V1 stack up  
2. Ingest or synthetic GitHub PR event → V1  
3. Project into V2 (bridge or projector)  
4. V3 compile ledger for actor twin  
5. (Post-shadow) DM draft via egress  
6. Veto or silence or publish  
7. Show channel post + metrics; show ACL revoke still hides private nodes  

---

## 11. What NOT to do in the next session

- Do not re-litigate Cockroach vs Neo4j (ADR-007 settled)  
- Do not re-open “build Buzz/Centaur” (ADR-011)  
- Do not put Slack tokens in twin env  
- Do not nest V3 crates inside `vertical-1/`  
- Do not invent product features outside the V3 TAS without a new ADR  

---

*End of handoff. Prefer git files over conversation memory.*
