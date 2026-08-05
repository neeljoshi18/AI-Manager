# TODO tomorrow morning (hotspot) — Neon observe DB

**Blocked without:** hotspot or DO Web Console (campus SSH often blocked).  
**Reminder:** scheduled in Grok (~daily from 2026-08-05).

## On droplet `206.189.129.31`

1. Web Console or `ssh neel@206.189.129.31` (hotspot).
2. `nano ~/ai-manager/deploy/.env.staging`
3. Add (never commit; never paste full URL in chat):

```bash
OBSERVE_DATABASE_URL=postgresql://…neon.tech/neondb?sslmode=require
```

4. Restart twin-api only:

```bash
cd ~/ai-manager
docker compose -f deploy/docker-compose.app.yml --env-file deploy/.env.staging --profile tls up -d --no-deps twin-api
```

5. Smoke:

```bash
curl -s https://status.neel.world/v3/tenants/ten_github/events | jq .external_db
# → true
```

6. Approve once in app; Neon SQL:

```sql
SELECT at, kind, subject FROM twin_events ORDER BY at DESC LIMIT 20;
```

See also: `plans/observe-neon.md`.
