# Deploy — M5 staging

Goal: run AI Manager on a **single host** a teammate can hit over HTTPS — without laptop sleep killing the demo.

## What exists

| Piece | Path |
|-------|------|
| Local full stack (host binaries) | `./scripts/dev_up.sh` |
| Infra only (Redis, CRDB, Redpanda, MinIO, CH) | `deploy/docker-compose.platform.yml` |
| **App services (V1+V2+V3+egress)** | `deploy/docker-compose.app.yml` |
| TLS edge (Caddy, profile `tls`) | `deploy/Caddyfile` |
| OAuth / GitHub App scaffolding | `deploy/oauth/` |
| Wake runbook | `plans/2026-07-23_wake-laptop-runbook.md` |
| Product UI | `http://HOST:18083/app/` |
| Lab | `http://HOST:18083/demo/` |

## Quick path — multi-service Docker (recommended staging)

```bash
# From monorepo root
# Ensure egress vault exists (never commit real tokens):
#   cp vertical-security/secrets/dev_secrets.example.json \
#      vertical-security/secrets/dev_secrets.json
#   # edit SLACK_BOT_TOKEN

docker compose -f deploy/docker-compose.app.yml up -d --build

# Product UI
open http://127.0.0.1:18083/app/
# Health / last-event age
curl -s http://127.0.0.1:18083/v3/demo/status | jq .
# Webhooks
curl -s http://127.0.0.1:18080/healthz | jq .
```

Embedded mode: no Cockroach required. Good for demos and first staging VPS.

Stop:

```bash
docker compose -f deploy/docker-compose.app.yml down
```

### Images

| Service | Dockerfile | Port |
|---------|------------|------|
| egress | `deploy/Dockerfile.egress` | 18090 |
| v1 | `deploy/Dockerfile.v1-ingestion` | 18080 |
| v2 | `deploy/Dockerfile.v2-graph-api` | 18082 |
| twin-api | `deploy/Dockerfile.twin-api` | 18083 |

Build one-off:

```bash
docker build -f deploy/Dockerfile.v1-ingestion -t ai-manager-v1 ./vertical-1
docker build -f deploy/Dockerfile.v2-graph-api -t ai-manager-v2 ./vertical-2
docker build -f deploy/Dockerfile.twin-api -t ai-manager-twin-api ./vertical-3
docker build -f deploy/Dockerfile.egress -t ai-manager-egress ./vertical-security
```

## HTTPS staging path

1. **Host** — small VPS (2 vCPU / 4GB+) with Docker; open 80/443 (and 18080 if exposing webhooks raw).
2. **DNS** — `A`/`AAAA` for e.g. `status.example.com` → VPS.
3. **Secrets** — `vertical-security/secrets/dev_secrets.json` on host (gitignored).
4. **Start with TLS profile:**

```bash
export DOMAIN=status.example.com
docker compose -f deploy/docker-compose.app.yml --profile tls up -d --build
```

Caddy terminates TLS and routes:

| Path | Backend |
|------|---------|
| `/v1/*` | V1 webhooks |
| everything else | twin-api (`/app/`, `/demo/`, `/v3/*`) |

5. **GitHub** — App webhook → `https://$DOMAIN/v1/tenants/{tenant}/webhooks/github` (see `deploy/oauth/`).
6. **Slack** — bot token only in egress vault; OAuth install when client id/secret available.

### Still need from human

- Cloud host account / DNS (if not local-only)
- Slack OAuth client id/secret (manifest ready in `deploy/oauth/slack-app-manifest.json`)
- GitHub App credentials (manifest ready in `deploy/oauth/github-app-manifest.yml`)

## Infra + production DBs (optional)

```bash
docker compose -f deploy/docker-compose.platform.yml up -d

# create DBs
docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
  ./cockroach sql --insecure -e "CREATE DATABASE IF NOT EXISTS context_graph; CREATE DATABASE IF NOT EXISTS status_twins;"

# migrate (from monorepo root)
docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
  ./cockroach sql --insecure -d defaultdb < vertical-1/migrations/cockroach/001_init.sql
docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
  ./cockroach sql --insecure -d context_graph < vertical-2/migrations/cockroach/001_init.sql
docker compose -f deploy/docker-compose.platform.yml exec -T cockroach \
  ./cockroach sql --insecure -d status_twins < vertical-3/migrations/cockroach/001_init.sql
```

Then run app images with `RUNTIME_MODE=production` and `COCKROACH_URL=…` (service hostname `cockroach` when on the same compose network). Default `docker-compose.app.yml` stays **embedded** for simplicity.

## Environment (production sketch)

```bash
# V3
RUNTIME_MODE=production
BIND_ADDR=0.0.0.0:18083
COCKROACH_URL=postgresql://root@cockroach:26257/status_twins?sslmode=disable
V1_BASE_URL=http://v1:18080
V2_BASE_URL=http://v2:18082
EGRESS_PROXY_URL=http://egress:18090
EGRESS_ENFORCE=true
USE_EGRESS_SLACK=true
STATUS_WINDOW_SECS=3600
NOTIFY_INTERVAL_SECS=1800
COMPILE_INTERVAL_SECS=1800
NOTIFY_ON_COMPILE=false

# V2
RUNTIME_MODE=production
COCKROACH_URL=postgresql://root@cockroach:26257/context_graph?sslmode=disable
V1_COCKROACH_URL=postgresql://root@cockroach:26257/defaultdb?sslmode=disable
```

## Security rules (never break)

- `SLACK_BOT_TOKEN` / long-lived GitHub tokens **only** in egress vault (ADR-012).
- Never put those on twin-api / worker env.
- Ingest continuous; Slack notify **batched** (ADR-014).
- No silent private 1:1 DM wiretap (ADR-015).

## Checklist

- [x] Infra compose
- [x] twin-api Dockerfile
- [x] Dockerfiles for V1 / V2 / egress
- [x] Multi-service app compose
- [x] Caddy HTTPS path scaffold
- [x] Connections last-event age (V1 → twin `/v3/demo/status` → `/app/` UI)
- [x] Slack / GitHub App manifests (secrets blocked on human)
- [ ] Host + DNS + real deploy credentials
- [ ] Slack OAuth install flow wired in twin-api
- [ ] GitHub App production install
- [ ] Multi-tenant control plane (M7)

## Onboarding (product path)

1. Create tenant  
2. Connect Slack (OAuth — scaffold only until secrets)  
3. Connect GitHub (App — scaffold only until secrets)  
4. Shadow mode N days  
5. First scheduled status DM  
6. Kill a standup  

UI: **Connections** in `/app/` shows service pills + **last event age** when V1 has accepted webhooks.
