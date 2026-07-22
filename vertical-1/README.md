# Vertical 1 — Enterprise Telemetry Ingestion, Canonical Data Engine & Real-Time ACL Mirroring

Ground-truth implementation of the **Technical Architecture Specification: Vertical 1** for the Autonomous AI Manager platform.

Vertical 1 is the immutable, high-throughput data foundation for later verticals (context graph, digital twins, status ledgers). It ingests webhook telemetry from developer exhaust systems, normalizes it into strongly typed canonical events, mirrors source ACLs, and serves ACL-filtered reads — **without** cloning proprietary source code or running full-text/vector search (the anti-Glean cost model).

---

## System invariants (non-negotiable)

| # | Invariant | How we enforce it |
|---|-----------|-------------------|
| 1 | **Zero data loss** | Durable bus append before HTTP 200 |
| 2 | **Zero-trust ACL isolation** | Query-time `is_private OR hasAny(groups)` filter on every read |
| 3 | **Sub-50ms P99 ingestion** | Edge path = auth → rate limit → dedup → enqueue only |
| 4 | **Deterministic type safety** | Protobuf contract + compile-time Rust types |

---

## Architecture

```
Webhooks (GitHub/GitLab/Jira/Linear/Slack/Teams/Zendesk)
        │
        ▼
┌───────────────────────────────────────┐
│  Ingestion Edge (Rust / Axum)         │
│  HMAC auth · Redis-style rate limit   │
│  Dedup (SET NX) · Object-store vault  │
│  Normalize → CanonicalEvent           │
└───────────────────────────────────────┘
        │ durable publish
        ▼
┌───────────────────────────────────────┐
│  Streaming Bus                        │
│  topics: events.raw / realtime /      │
│          backfill / acl               │
│  Prod: Redpanda (Kafka API)           │
│  Local: InMemoryBus (ordered log)     │
└───────────────────────────────────────┘
        │
        ├──────────────► Analytical Store (ClickHouse / embedded)
        │                ReplacingMergeTree-style dedup
        │                Query-time ACL filter
        │
        └──────────────► ACL Store (CockroachDB / embedded)
                         Identity map + GroupMap + Pub/Sub invalidation
```

### Competitive context (why this shape)

From the Glean competitive analysis: Glean’s cost is dominated by full-text indexing, vector hosting, OCR, and elevated admin scopes. Vertical 1 **only** stores collaborative metadata (commits, PR metrics, ticket status, short text previews, group membership). Raw JSON is vaulted to object storage; agents in later verticals never get unauthorized rows because ACL is enforced at query time, not “trust the LLM.”

---

## Repository layout

```
vertical-1/
├── proto/enterprise/telemetry/v1/events.proto   # Canonical Protobuf contract
├── crates/
│   ├── telemetry-proto/       # prost-generated types
│   ├── telemetry-core/        # domain, backends, pipeline, normalizers
│   ├── telemetry-ingestion/   # Axum webhook edge (+ embedded query API)
│   ├── telemetry-consumer/    # Bus → analytical store worker
│   ├── telemetry-query/       # Standalone ACL query service
│   └── telemetry-verify/      # Spec §5 verification battery (TC-01..TC-06)
├── migrations/
│   ├── clickhouse/001_init.sql
│   └── cockroach/001_init.sql
├── docker-compose.yml         # Redis, Redpanda, CockroachDB, ClickHouse, MinIO
├── scripts/smoke_http.sh
├── scripts/egress_smoke.sh   # curl egress proxy if already up
└── Makefile
```

### Outbound credential egress (optional)

Long-lived API tokens for outbound calls (GitHub REST backfill, etc.) should **not** live in the worker process. Use the Centaur-inspired proxy in `../vertical-security/`:

| Env | Meaning |
|-----|---------|
| `EGRESS_PROXY_URL` | e.g. `http://127.0.0.1:18090` — route via `telemetry_core::egress::EgressClient` |
| `EGRESS_ENFORCE` | `true` → fail closed if proxy unset (no env token fallback) |
| `SECRETS_FILE` | JSON name→value map; optional `WEBHOOK_SECRET_<tenant>` overlays |

```bash
# terminal A
cd ../vertical-security && cp secrets/dev_secrets.example.json secrets/dev_secrets.json && cargo run

# terminal B
./scripts/egress_smoke.sh
```

Inbound webhook HMAC is unchanged (in-process). See `../vertical-security/README.md`.


---

## Quick start (embedded mode — no Docker required)

```bash
# Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

cd vertical-1

# Unit tests + Spec verification battery
make test
# or:
cargo test -p telemetry-core --lib
cargo run -p telemetry-verify -- --load-count 5000 --replay-count 10000 --workers 50

# Run the edge (ingestion + query on one process)
SKIP_AUTH=true RUNTIME_MODE=embedded cargo run -p telemetry-ingestion
# → http://127.0.0.1:18080  (8080 avoided — often taken by local tools)
```

### HTTP smoke

```bash
# terminal 1
SKIP_AUTH=true cargo run -p telemetry-ingestion

# terminal 2
chmod +x scripts/smoke_http.sh
./scripts/smoke_http.sh
```

### Production-parity infra (when Docker is available)

```bash
docker compose up -d
cp .env.example .env
# Set RUNTIME_MODE=production and backend URLs, then:
cargo run -p telemetry-ingestion
cargo run -p telemetry-consumer
cargo run -p telemetry-query
```

---

## API surface

### Tenant + webhooks

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/v1/tenants` | Upsert tenant + webhook secrets + default groups |
| `POST` | `/v1/tenants/{tenant}/webhooks/{provider}` | Ingest signed webhook |
| `GET`  | `/healthz` `/readyz` `/metrics` | Liveness / readiness / percentiles |

Providers: `github` `gitlab` `jira` `linear` `slack` `teams` `zendesk`

### ACL-filtered query (Vertical 2+ entrypoint)

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/v1/tenants/{tenant}/events?user_id=…` | ACL-filtered event list |
| `GET` | `/v1/tenants/{tenant}/resource-state?user_id=…&resource_id=…` | Origin-time state reconstruction |
| `POST` | `/v1/tenants/{tenant}/users` | Seed identity + groups |
| `PUT/DELETE` | `/v1/tenants/{tenant}/users/{uid}/groups/{gid}` | Membership change / revocation |

---

## Verification battery (Spec §5) — internal results

Run: `cargo run -p telemetry-verify`

| ID | Scenario | Local result |
|----|----------|--------------|
| **TC-01** | Load (5k concurrent ingests) | PASS — 0 drops, P99 ≪ 50ms embedded |
| **TC-02** | 10k identical delivery IDs | PASS — exactly 1 analytical record |
| **TC-03** | ACL revocation + 1k queries | PASS — 0 leaks, revoke &lt; 200ms |
| **TC-04** | Schema mutations / bad JSON | PASS — no crash; hard failures → DLQ |
| **TC-05** | Concurrent bus durability | PASS — published == received |
| **TC-06** | Out-of-order PR closed/opened | PASS — derived state `CLOSED` |

### What was tested internally vs what you still need

**Tested here (automated):**

- HMAC verify (GitHub + Slack) with constant-time compare  
- Rate limit per tenant  
- Dedup / idempotency  
- All 7 provider normalizers (unit + structural)  
- ACL allow/deny + revocation propagation  
- Out-of-order state reconstruction  
- Durable bus under concurrency  
- Dead-letter on malformed payloads  
- Metadata-only attributes (no patch/diff storage)

**Not fully testable without your environment (manual / staging):**

1. Real GitHub/GitLab/Jira/Slack webhook delivery + signature secrets  
2. Envoy TLS termination + edge rate limits in front of Axum  
3. Redpanda multi-broker kill (TC-05 chaos at cluster level)  
4. ClickHouse ReplacingMergeTree background merges at multi-million row scale  
5. CockroachDB multi-region serializability under partition  
6. Redis cluster failover during dedup (Tier-1) → Tier-2 ClickHouse dedup catch  
7. 25,000 req/sec spike on GKE (TC-01 full prod scale)  
8. Historical backfill adaptive rate limiting against live GitHub/Jira quotas  
9. MinIO/S3 encryption-at-rest + bucket policies  
10. PagerDuty / Grafana dashboard wiring  
11. Cross-vertical handoff (Vertical 2 Organizational Context Graph consumers)

---

## Manual test plan (product readiness)

Use this checklist to decide “Vertical 1 is ready to unblock Vertical 2.”

### A. Happy path

1. Register a tenant with a GitHub webhook secret.  
2. Seed a user into `grp_eng_core`.  
3. Send a signed `pull_request.opened` webhook for a **private** repo.  
4. Query `/events` as that user → expect 1 event; attributes contain title/state, **not** file diffs.  
5. Query resource-state for `org/repo/pr/N` → `OPEN`.

### B. Security / ACL edge cases

6. Seed a second user with **no** groups (or only `grp_sales`).  
7. Query as second user → **count = 0** (no leakage).  
8. Remove first user from `grp_eng_core`; immediately re-query 100× → always empty.  
9. Re-add group → event visible again.  
10. Public repo event (`private: false`) → visible to users with empty groups.

### C. Idempotency & ordering

11. Replay the same `X-GitHub-Delivery` 100× → single record, all 200 OK.  
12. Send `pull_request.closed` (T2) then `pull_request.opened` (T1) → state `CLOSED`.  
13. Send GitHub `push` with many commits → only SHAs + messages stored (no patches).

### D. Provider matrix

14. GitLab merge_request webhook.  
15. Jira `jira:issue_updated` with changelog.  
16. Linear Issue update.  
17. Slack `event_callback` message (and `url_verification` challenge).  
18. Teams activity message.  
19. Zendesk ticket update.

### E. Failure modes

20. Wrong HMAC → 401, nothing stored.  
21. Unknown tenant → 404.  
22. Burst past rate limit → 429 + `Retry-After`.  
23. Invalid JSON body → 202 / dead-lettered, service stays up.  
24. Extra unknown JSON fields → still accepted (forward compatible).  
25. Missing optional PR fields → accepted with defaults.

### F. Operability

26. `/metrics` shows rising `accepted`, stable p99.  
27. `/readyz` returns runtime mode + snapshot.  
28. (With Docker) kill Redpanda, confirm compose health recovery + no silent drops once back.  
29. (With Docker) inspect ClickHouse row after consumer write.  
30. Confirm raw payload URI exists in MinIO/local object store.

### G. Compliance posture (AI Manager pitch)

31. Confirm no source file contents in `attributes_json`.  
32. Confirm Slack/Teams only keep ≤280 char previews.  
33. Confirm identity events update group membership used by later queries.

---

## Definition of Done mapping (Spec §6)

| Exit criterion | Status |
|----------------|--------|
| Infra via Terraform to staging | **Partial** — `docker-compose.yml` + SQL migrations; Terraform not yet |
| Protobuf contract locked | **Done** — `proto/…/events.proto` + prost compile |
| TC-01…TC-06 100% pass | **Done** in embedded mode; prod-scale/chaos deferred |
| Grafana observability | **Partial** — `/metrics` JSON snapshot; Grafana dashboards TBD |
| Security review (HMAC + ACL) | **Ready for review** — implemented; external sign-off pending |

---

## Configuration

See `.env.example`. Key vars:

- `RUNTIME_MODE=embedded|production`  
- `SKIP_AUTH=true` — local only  
- `RATE_LIMIT_PER_MINUTE=10000` (spec default)  
- `DEDUP_TTL_SECS=86400`  
- Backend URLs for Redis / Kafka / Cockroach / ClickHouse / S3 when not embedded  

---

## Next verticals

Vertical 1 unblocks:

- **Vertical 2** — Organizational Context Graph (time, lineage, dependencies over this telemetry)  
- **Vertical 3** — Digital twins / agent negotiation reading ACL-filtered context  
- Status ledgers, veto-first Slack DMs, confidence tiers  

Those layers **must** call the query API with a real `QueryContext`; they must never bypass ACL.
