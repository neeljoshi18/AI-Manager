# Durability + fast deploy (2026-08-03)

## Why graph looked “gone”

| Layer | Truth |
|-------|--------|
| V2 graph | **Docker volume** `ai-manager_v2_state` → `v2_graph.json` **did** load (13 nodes / 15 edges after last deploy) |
| V1 events | Path set but flush was **every 5 writes** and no shutdown flush → often **no `v1_events.json`** → bridge cannot re-project after wipe |
| V2 membership | **In-memory only** → groups lost every restart → `no_neighborhood` until ensure_users |
| Hard refresh | Browser cache only; does **not** wipe volumes |

## Fixes shipped

1. V1 events + ACL: **flush every write**
2. V2 graph: **flush every write**
3. V2 membership: **disk journal** `v2_membership.json`
4. `GET /v2/durability` + demo_status.durability
5. BuildKit **cargo cache** Dockerfiles (rebuilds minutes, not 20m cold)
6. `deploy/scripts/deploy_fast.sh` — rebuild only changed services

## Sales timeline

| Day | Bar |
|-----|-----|
| Now | Stack live; multi_person true; neel digests with items (7d window) |
| +1 deploy of this commit | Durability airtight + faster rebuilds |
| Day-after-tomorrow sales | Soft outreach only after `pilot_readiness.soft_outreach_ready` or founder accepts solo+map demo |

## Confirm after deploy

```bash
ssh neel@DROPLET 'docker run --rm -v ai-manager_v2_state:/d alpine ls -la /d'
# expect v2_graph.json + v2_membership.json growing after ensure_users
curl -sS https://status.neel.world/v3/demo/status | jq .durability
# restart v2 only, then graph node count should match
```
