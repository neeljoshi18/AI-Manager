# Live observability DB (Neon free tier)

## Why

Product actions (Approve / Don't send / compile) used to feel invisible.
`GET /v3/tenants/{tenant}/events` always returns an **embedded** ring log.
Set `OBSERVE_DATABASE_URL` to mirror every event into **Postgres** so you can:

```sql
SELECT * FROM twin_events ORDER BY at DESC LIMIT 50;
```

and watch ingestion + approve in real time.

## Pick: Neon

[Neon](https://neon.tech) free tier: serverless Postgres, enough for pilot event logs.

## Steps (you do once)

1. Create account at https://console.neon.tech  
2. New project → copy the connection string  
   (`postgresql://user:pass@ep-….aws.neon.tech/neondb?sslmode=require`)  
3. On the droplet, add to `deploy/.env.staging` (never commit):

```bash
OBSERVE_DATABASE_URL=postgresql://USER:PASSWORD@HOST/neondb?sslmode=require
```

4. Redeploy twin-api (Actions → Deploy staging → `recover_only=true` is enough).  
5. twin-api auto-creates table `twin_events` on boot.  
6. Click **Approve** or **Don't send**, then:

```bash
curl -s https://status.neel.world/v3/tenants/ten_github/events | jq .
# or in Neon SQL editor:
# SELECT at, kind, subject, detail FROM twin_events ORDER BY at DESC LIMIT 30;
```

7. Product UI: **Settings → Live event trail → Refresh events**.

## What gets logged

| kind | when |
|------|------|
| `approve` | My status Approve / Slack text / interaction |
| `approve_already` | Approve when already published |
| `approve_failed` | Egress failure |
| `dont_send` | Don't send / veto |
| (more later) | compile, ingest hooks |

## Without Neon

Embedded log still works (twin state JSON / in-memory). External DB is optional gas for you as operator.

## Full twin mirror (2026-08-06) + continuous dual-write

Tables: `twin_events`, `twin_snapshot_json`, `twin_twins`, `twin_slack_maps`, `twin_drafts`, `twin_tenant_kv`.

- `GET /v3/observe/status` — `external_db` + `continuous_mirror` + `graph_mirror`
- `POST /v3/tenants/{tenant}/sync_to_db` — optional bulk **upsert** (not required daily)
- Every `persist_embedded` dual-writes twin state to Neon when connected
- UI: Settings → **Force full re-sync → Neon** (optional)

Prefer GitHub secret `OBSERVE_DATABASE_URL` (see `plans/2026-08-06_neon-you-do-this.md`).

## V2 graph snapshot → Neon (SQL insights)

Tables: `graph_nodes`, `graph_edges`, `graph_export_meta`.

- `POST /v3/tenants/{tenant}/sync_graph_to_db` — fetch V2 ACL snapshot (`bridge_reader`, node_limit=2000, edge_limit=5000), upsert + delete orphans
- Background loop when Neon connected: every `GRAPH_NEON_EXPORT_INTERVAL_SECS` (default **900**), default tenant
- Edge ids: use V2 `id` when present; else `{type}:{from}->{to}:{valid_from}`
- UI: Settings → **Export graph → Neon** (on-demand counts in events-meta)
- Graph UI remains primary; Neon is for SQL analytics only

Example:

```sql
SELECT node_type, count(*) FROM graph_nodes WHERE tenant_id = 'ten_github' GROUP BY 1;
SELECT edge_type, count(*) FROM graph_edges WHERE tenant_id = 'ten_github' GROUP BY 1;
SELECT * FROM graph_export_meta;
```

### What is / is not migrated from Docker

| In Neon | Still on droplet volumes only |
|---------|-------------------------------|
| Twins, Slack maps, digests, kv, events, twin snapshot JSON | V1 event journal (not migrated), bridge cursors, vault secrets |
| V2 graph nodes/edges (periodic + on-demand export into `graph_*`) | Raw V2 graph JSON on volume (export is a SQL mirror, not live replacement) |

Full “stateless containers, everything in Neon” = later multi-tenant packaging. Graph UI already shows work data without Neon.
