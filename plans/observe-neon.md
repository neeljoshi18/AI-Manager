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

## Full twin mirror (2026-08-06)

Tables: `twin_events`, `twin_snapshot_json`, `twin_twins`, `twin_slack_maps`, `twin_drafts`, `twin_tenant_kv`.

- `GET /v3/observe/status`
- `POST /v3/tenants/{tenant}/sync_to_db` — Docker volume → Neon
- UI: Settings → **Mirror twin state → Neon**

Prefer GitHub secret `OBSERVE_DATABASE_URL` (see `plans/2026-08-06_neon-you-do-this.md`).
