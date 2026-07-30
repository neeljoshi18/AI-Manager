# Plan: Embedded durability + intent blockers demo

**Date:** 2026-07-30  
**Why graph emptied:** Staging runs `RUNTIME_MODE=embedded` for V1/V2/V3. In-memory stores wipe on container restart/rebuild. Bridge re-project needs V1 events; if V1 also wiped, graph stays empty until new webhooks.

## Fix shipped

| Store | Env | Volume |
|-------|-----|--------|
| V1 events | `V1_EMBEDDED_STATE_PATH` | `v1_state` |
| V2 graph | `GRAPH_EMBEDDED_STATE_PATH` | `v2_state` |
| V3 twins | `TWIN_EMBEDDED_STATE_PATH` | `twin_state` |

Long-term: Cockroach `context_graph` + `status_twins` (platform compose) for production tenants.

## Intent prototype

- `POST /v2/tenants/{t}/seed/intent_demo` — SHIP vs FREEZE dual owners + BLOCKED + BLOCKS
- `POST /v3/tenants/{t}/seed/intent_demo` — product proxy
- UI: Today → **Load intent demo**

## Deploy

```bash
./deploy/scripts/sync_and_deploy_staging.sh
# then seed:
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/seed/intent_demo
```
