# Vertical 2 — Organizational Context Graph

**Status:** Implemented (embedded + production Cockroach).

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

**Live ACL:** with `V1_COCKROACH_URL`, graph reads use Vertical 1 `user_group_membership` so V1 group revoke immediately hides private graph nodes (no dual-write lag).

## Project a V1 event

```bash
# After V1 ingest, POST the same canonical event JSON:
curl -s http://127.0.0.1:18080/v1/tenants/TEN/events?user_id=USER | jq '.events[0]' \
  | curl -s -X POST http://127.0.0.1:18082/v2/project -H 'content-type: application/json' -d @-
```

Optional bus consumer: `cargo run -p graph-projector` (Redpanda → graph).

## Stack

| Layer | Choice |
|-------|--------|
| Graph SoT | CockroachDB `context_graph` |
| Live groups | V1 `defaultdb.user_group_membership` (hybrid) |
| Runtime | Rust / Axum |
| Input | V1 event JSON / Redpanda |
