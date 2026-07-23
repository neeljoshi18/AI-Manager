# Vertical 3 — Status Twins, Ledgers & Veto-First Delivery

**Status:** Implemented (embedded + production Cockroach). TC-T01–TC-T10 green. **Demo console** at `/demo/` (M4 Sew & Show).

## What it is

Compiles **status ledgers** from Vertical 2’s ACL-safe graph, delivers them **privately first** (Slack DM), respects **confidence tiers** and **human veto**, then publishes to the team channel. This is the meeting-killing product layer.

## Spec (ground truth)

**[Technical Architecture Specification_ Vertical 3.md](./Technical%20Architecture%20Specification_%20Vertical%203.md)**

Stick to that document. If ambiguous, prefer §1.2 invariants.

### §1.2 invariants (enforced)

| # | Invariant | How |
|---|-----------|-----|
| 1 | ACL never bypassed | Compiler uses V2 HTTP / fixtures only — no god-mode graph SQL |
| 2 | Veto-first | Channel post only after edit/veto opportunity (Medium silence = consent) |
| 3 | Confidence tiers | Deterministic High / Medium / Blocker |
| 4 | Evidence-backed | Every item has `evidence_refs` (event/edge/node) |
| 5 | Egress-only secrets | Slack via `:18090` `slack_api`; never `SLACK_BOT_TOKEN` in twin env |
| 6 | No upward coupling | V3 depends on V2 + egress; V1/V2 do not depend on V3 |
| 7 | Metadata posture | Short summaries only |
| 8 | Tenant isolation | All keys include `tenant_id` |
| 9 | No surveillance | No LOC rankings / productivity scores |

## Layout

```
vertical-3/
├── Technical Architecture Specification_ Vertical 3.md
├── README.md
├── Cargo.toml
├── crates/
│   ├── twin-core/       # domain: ledger, confidence, state machine, store
│   ├── twin-compiler/   # V2 client + fixture graph → ledger_snapshot
│   ├── twin-delivery/   # DM + veto worker + mock/egress Slack
│   ├── twin-api/        # Axum :18083
│   └── twin-verify/     # TC-T01…T10
├── migrations/cockroach/
│   └── 001_init.sql
└── scripts/
    ├── smoke_v3.sh
    └── sew_e2e.sh
```

## Quick start (embedded — no Docker)

```bash
cd vertical-3
cargo test
cargo run -p twin-verify          # TC-T01…T10
RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 cargo run -p twin-api   # :18083
# Product UI (light B&W):  http://127.0.0.1:18083/app/
# Lab console:             http://127.0.0.1:18083/demo/
# Or from monorepo root:   ../scripts/dev_up.sh
./scripts/smoke_v3.sh
../scripts/platform_sew.sh
```

### Real Slack DMs

```bash
# terminal A — egress (token only in secrets file, not twin env)
cd ../vertical-security && cargo run -- --bind 0.0.0.0:18090 \
  --registry config/tool_registry.yaml --secrets secrets/dev_secrets.json

# terminal B
cd ../vertical-3
USE_EGRESS_SLACK=true EGRESS_PROXY_URL=http://127.0.0.1:18090 EGRESS_ENFORCE=true \
  RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 cargo run -p twin-api
```

Then in demo console set your Slack **member ID** (`U…`) and Simulate.

## Production (shared V1 Docker stack)

```bash
# from monorepo root — V1 compose already running
docker compose -f vertical-1/docker-compose.yml exec -T cockroach \
  ./cockroach sql --insecure -e "CREATE DATABASE IF NOT EXISTS status_twins;"
docker compose -f vertical-1/docker-compose.yml exec -T cockroach \
  ./cockroach sql --insecure -d status_twins < vertical-3/migrations/cockroach/001_init.sql

cd vertical-3
RUNTIME_MODE=production \
  COCKROACH_URL='postgresql://root@127.0.0.1:26257/status_twins?sslmode=disable' \
  V2_BASE_URL='http://127.0.0.1:18082' \
  EGRESS_PROXY_URL='http://127.0.0.1:18090' \
  EGRESS_ENFORCE=true \
  cargo run -p twin-api
```

**Slack:** run `vertical-security` egress-proxy on `:18090` with `SLACK_BOT_TOKEN` in the vault file — never in twin env.

## Ports

| Service | Port |
|---------|------|
| V1 | 18080 |
| V2 | 18082 |
| **V3 twin-api** | **18083** |
| Egress proxy | 18090 |

## Env

| Variable | Purpose |
|----------|---------|
| `RUNTIME_MODE` | `embedded` (default) \| `production` |
| `BIND_ADDR` | default `0.0.0.0:18083` |
| `COCKROACH_URL` | `status_twins` database (production) |
| `V2_BASE_URL` | e.g. `http://127.0.0.1:18082` |
| `EGRESS_PROXY_URL` | e.g. `http://127.0.0.1:18090` |
| `EGRESS_ENFORCE` | fail closed if proxy unset (default true) |
| `SKIP_AUTH` | local only |
| `SHADOW_MODE_DAYS` | default 10 |
| `USE_EGRESS_SLACK` | embedded: use real egress client instead of mock |

## API surface (`:18083`)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/healthz` `/readyz` `/metrics` | Ops |
| POST | `/v3/tenants/{t}/twins` | Upsert twin config |
| GET | `/v3/tenants/{t}/twins/{twin_id}` | Get twin |
| POST | `/v3/tenants/{t}/twins/{twin_id}/compile` | On-demand compile + delivery start |
| GET | `/v3/tenants/{t}/ledgers/{ledger_id}` | Ledger snapshot |
| GET | `/v3/tenants/{t}/drafts/{draft_id}` | Draft status |
| POST | `/v3/tenants/{t}/drafts/{draft_id}/veto` | Veto |
| POST | `/v3/tenants/{t}/drafts/{draft_id}/edit` | Edit body |
| POST | `/v3/tenants/{t}/drafts/{draft_id}/publish` | Force publish |
| POST | `/v3/tenants/{t}/drafts/{draft_id}/silence` | Medium silence timeout |
| POST | `/v3/slack/interactions` | Slack interactivity |
| POST | `/v3/tenants/{t}/fixtures` | Embedded only: inject ACL-filtered graph |

Compile body may include `"skip_shadow": true` for local demos.

## Metrics

`GET /metrics` returns: `twin_compile_total_ok/error`, `twin_drafts_sent_total`, `twin_veto_total`, `twin_publish_total_ok/fail`, `twin_acl_empty_total`, `twin_egress_fail_total`.

## Verification

| ID | Scenario |
|----|----------|
| TC-T01 | Synthetic fixtures → ledger items + evidence |
| TC-T02 | High + auto-publish → `publish_record` |
| TC-T03 | Medium DM + silence → channel post |
| TC-T04 | Veto → no publish |
| TC-T05 | Edit → body_hash matches |
| TC-T06 | ACL: private PR not leaked |
| TC-T07 | No Slack token in twin env |
| TC-T08 | Exactly-once publish |
| TC-T09 | Shadow: no Slack |
| TC-T10 | Sew chain (fixture V1→V2→V3) |

## Product stance

- Not Glean (no search index)
- Not Buzz (no workspace/Git forge)
- Not Centaur agent OS (egress inject only)
- No individual productivity rankings

## Related docs

- `../starting-out-documents/Session Handoff_ AI Manager State.md`
- `../starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` (ADR-013)
- `../vertical-2/Technical Architecture Specification_ Vertical 2.md`
- `../vertical-security/README.md`
