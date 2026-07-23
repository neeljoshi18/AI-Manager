# Deploy — M5 staging foundation

Goal: run AI Manager on a **single host** a teammate can hit over HTTPS — without laptop sleep killing the demo.

## What exists today

| Piece | Path |
|-------|------|
| Local full stack | `./scripts/dev_up.sh` |
| Infra compose | `deploy/docker-compose.platform.yml` |
| Wake runbook | `plans/2026-07-23_wake-laptop-runbook.md` |
| Product UI | `http://HOST:18083/app/` |
| Lab | `http://HOST:18083/demo/` |

## Staging checklist

1. **Host** — small VPS (2 vCPU / 4GB) or Fly/Render (future Dockerfiles per service).  
2. **Infra** — `docker compose -f deploy/docker-compose.platform.yml up -d`  
3. **Databases**
   ```bash
   # create DBs
   docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
     ./cockroach sql --insecure -e "CREATE DATABASE IF NOT EXISTS context_graph; CREATE DATABASE IF NOT EXISTS status_twins;"
   # migrate
   docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
     ./cockroach sql --insecure -d context_graph < vertical-2/migrations/cockroach/001_init.sql
   docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
     ./cockroach sql --insecure -d status_twins < vertical-3/migrations/cockroach/001_init.sql
   ```
4. **Secrets** — `vertical-security/secrets/dev_secrets.json` on host (not git).  
5. **Services** — run V1/V2/V3/egress with `RUNTIME_MODE=production` + `COCKROACH_URL` or keep embedded for tiny demos.  
6. **TLS** — Caddy/nginx reverse proxy to `:18083` (app) and optionally `:18080` (webhooks).  
7. **GitHub** — prefer **GitHub App** pointing at `https://your.domain/v1/tenants/{t}/webhooks/github`.  
8. **Slack** — OAuth install (scopes: `chat:write`, `im:write`, optional events). Bot token only in egress vault.

## Environment (production sketch)

```bash
# V3
RUNTIME_MODE=production
BIND_ADDR=0.0.0.0:18083
COCKROACH_URL=postgresql://root@127.0.0.1:26257/status_twins?sslmode=disable
V2_BASE_URL=http://127.0.0.1:18082
EGRESS_PROXY_URL=http://127.0.0.1:18090
EGRESS_ENFORCE=true
STATUS_WINDOW_SECS=3600
NOTIFY_INTERVAL_SECS=1800
COMPILE_INTERVAL_SECS=1800
NOTIFY_ON_COMPILE=false

# V2
RUNTIME_MODE=production
COCKROACH_URL=postgresql://root@127.0.0.1:26257/context_graph?sslmode=disable
V1_COCKROACH_URL=postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable
```

## Not yet (M5 remaining)

- [ ] Multi-stage Dockerfiles for each Rust binary  
- [ ] Slack OAuth install flow in UI  
- [ ] GitHub App manifest + install  
- [ ] Managed TLS automation  
- [ ] Multi-tenant control plane  

## Onboarding (product path)

1. Create tenant  
2. Connect Slack (OAuth)  
3. Connect GitHub (App)  
4. Shadow mode N days  
5. First scheduled status DM  
6. Kill a standup  

UI stub for this lives under **Connections** in `/app/` (manual IDs today; OAuth next).
