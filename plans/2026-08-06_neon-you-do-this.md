# Neon setup — what **you** do (hotspot / video script)

I (agent) already built: tables, dual-write events, **Mirror twin state → Neon**, Actions inject from secret.

## Option A (recommended, 60 seconds) — GitHub Secret

1. Open: https://github.com/neeljoshi18/AI-Manager/settings/secrets/actions  
2. **New repository secret**  
   - Name: `OBSERVE_DATABASE_URL`  
   - Value: paste full Neon connection string  
     (must include `?sslmode=require` or Neon’s SSL params)  
3. Tell the agent: **“secret is set”**  
4. Agent runs recover deploy + `POST …/sync_to_db`

## Option B — Droplet file (Web Console / SSH)

1. DigitalOcean → droplet → **Web Console** (or `ssh neel@206.189.129.31` on hotspot)  
2. Run:

```bash
nano ~/ai-manager/deploy/.env.staging
```

3. Add one line at the bottom (paste your string, no quotes unless needed):

```bash
OBSERVE_DATABASE_URL=postgresql://USER:PASSWORD@ep-XXXX.region.aws.neon.tech/neondb?sslmode=require
```

4. Save (`Ctrl+O`, Enter, `Ctrl+X`)  
5. Tell agent: **“env file updated”**

## After either option — verify (you or agent)

```bash
curl -s https://status.neel.world/v3/observe/status | jq .
# external_db: true

curl -s -X POST https://status.neel.world/v3/tenants/ten_github/sync_to_db | jq .
# twins / drafts / maps counts

# Neon SQL Editor:
SELECT count(*) FROM twin_twins;
SELECT count(*) FROM twin_drafts;
SELECT kind, subject, at FROM twin_events ORDER BY at DESC LIMIT 20;
SELECT tenant_id, synced_at FROM twin_snapshot_json;
```

Product UI: **Settings → Live event trail + Neon DB → Mirror twin state → Neon**

## Do **not** paste the full connection string in chat.
