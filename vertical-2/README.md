# Vertical 2 — Organizational Context Graph

**Status:** Implemented (embedded + production Cockroach). Phase V2-A: dual-topic projector + offset commits.

## What it is

Projects **Vertical 1** telemetry into a temporal, ACL-safe property graph (people, PRs, repos, blockers, multi-hop paths). No webhooks — V1 is the only ingest plane.

## Spec

[Technical Architecture Specification_ Vertical 2.md](./Technical%20Architecture%20Specification_%20Vertical%202.md)

## Quick start (embedded — no Docker)

```bash
cd vertical-2
cargo run -p graph-verify          # TC-G01…G10
RUNTIME_MODE=embedded cargo run -p graph-api   # :18082
./scripts/smoke_v2.sh
# Optional V1↔V2 HTTP bridge demo:
./scripts/integration_v1_bridge.sh
```

## Production (shared V1 Docker stack)

```bash
# from monorepo root — V1 compose already running
docker compose -f vertical-1/docker-compose.yml exec -T cockroach \
  ./cockroach sql --insecure -e "CREATE DATABASE IF NOT EXISTS context_graph;"
docker compose -f vertical-1/docker-compose.yml exec -T cockroach \
  ./cockroach sql --insecure -d context_graph < vertical-2/migrations/cockroach/001_init.sql

cd vertical-2
RUNTIME_MODE=production \
  COCKROACH_URL='postgresql://root@127.0.0.1:26257/context_graph?sslmode=disable' \
  V1_COCKROACH_URL='postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable' \
  cargo run -p graph-api
```

**Live ACL:** with `V1_COCKROACH_URL`, graph reads use Vertical 1 `user_group_membership` so V1 group revoke immediately hides private graph nodes (no dual-write lag). Production `graph-api` always wraps membership in `HybridMembership`.

## Project a V1 event (HTTP bridge)

```bash
# After V1 ingest, POST the same canonical event JSON:
curl -s http://127.0.0.1:18080/v1/tenants/TEN/events?user_id=USER | jq '.events[0]' \
  | curl -s -X POST http://127.0.0.1:18082/v2/project -H 'content-type: application/json' -d @-
```

Or run `./scripts/integration_v1_bridge.sh` (V1 `:18080`, V2 `:18082`).

## Bus projector (Redpanda)

```bash
RUNTIME_MODE=production \
  COCKROACH_URL='postgresql://root@127.0.0.1:26257/context_graph?sslmode=disable' \
  V1_COCKROACH_URL='postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable' \
  KAFKA_BROKERS='127.0.0.1:19092' \
  KAFKA_TOPICS='events.raw,events.acl' \
  CONSUMER_GROUP='v2-graph-projector' \
  cargo run -p graph-projector
```

### Offset behavior

| Mode | Offsets |
|------|---------|
| `RUNTIME_MODE=embedded` | In-memory only (lost on restart; starts at broker Earliest) |
| `RUNTIME_MODE=production` | Persisted in Cockroach `projector_offsets` |

- **Start:** load `(consumer_group, topic, partition_id)` → `next_offset`; if missing, use broker **Earliest**.
- **Commit:** after a successful project (or intentional skip of bad payload), write `next_offset = record.offset + 1`.
- **Apply failure:** offset is **not** advanced so the message can be retried.
- **Bad payload:** log + skip + advance (poison-pill protection; process does not crash).

### Dual-topic consume

Default topics: `events.raw` and `events.acl` (CLI `--topics` or env `KAFKA_TOPICS`).  
Legacy single-topic env `KAFKA_TOPIC` is still accepted when `KAFKA_TOPICS` is unset.

Each topic runs a concurrent partition-0 consumer. Accepts V1 bus envelope (`V1BusMessage`) or bare `CanonicalEvent` / ACL revocation JSON.

Production projector also uses **HybridMembership** (same as graph-api) when `V1_COCKROACH_URL` is set.

## Metrics

`GET /metrics` on graph-api returns JSON counters: `projects_applied`, `projects_duplicate`, `projects_skipped`, `projects_error`, `acl_revocations`.

## Mappers (high level)

| Event | Graph effect |
|-------|----------------|
| `pull_request.*` / merge_request | Person, PR, Repo; AUTHORED, BELONGS_TO; lifecycle OPEN/CLOSED/**MERGED** |
| `issue.*` / jira / linear | Issue + AUTHORED; **ASSIGNED_TO** Issue→Person when assignee present |
| identity / member + team | **MEMBER_OF** Person→Team (+ ACL membership side channel) |
| slack / teams | Channel + DISCUSSED_IN |
| block hints | BLOCKS edges |

## Stack

| Layer | Choice |
|-------|--------|
| Graph SoT | CockroachDB `context_graph` |
| Live groups | V1 `defaultdb.user_group_membership` (hybrid) |
| Runtime | Rust / Axum |
| Input | V1 event JSON / Redpanda `events.raw` + `events.acl` |
| Offsets | `projector_offsets` (production) |
