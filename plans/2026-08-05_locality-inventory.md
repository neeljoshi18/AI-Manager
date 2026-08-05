# Locality inventory (path to “nothing is local”)

**Goal:** Another team can run the product without laptop state. Staging already runs on DO; some **data planes** are still droplet-local volumes.

| Plane | Today (staging) | Local laptop? | Next for portable multi-team |
|-------|-----------------|---------------|------------------------------|
| App code | Docker images on droplet, deploy via Actions | No (source only) | Same |
| twin-api process | Container | No | Same |
| V1 events | Volume `v1_state` on droplet | No | Object store / Postgres later |
| V2 graph | Volume `v2_state` | No | Managed graph DB or export |
| Twin digests/team | Volume `twin_state` JSON | No | CRDB/Postgres (production mode) |
| Event trail | Embedded + optional Neon | No after Neon | **Neon tomorrow (hotspot)** |
| Secrets | Host vault file | No | Vault/DO secrets |
| Slack/GitHub OAuth | Staging env on host | No | Per-tenant packaging |
| Your laptop | Dev only | Optional | Never required for pilot |

**Laptop is not in the serving path.** Hotspot is only for occasional SSH when campus blocks port 22; day-to-day = git push → Actions.
