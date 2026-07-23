# Plan execution — M5 multi-service compose + Connections last-event

**Date:** 2026-07-23  
**Parent:** `plans/2026-07-23_demo-to-product-m5.md`  
**Handoff §7 slice:** multi-service Docker; staging HTTPS path; Connections last-event age; OAuth scaffolding

## Done this build

### Multi-service Docker

- `deploy/Dockerfile.v1-ingestion`
- `deploy/Dockerfile.v2-graph-api`
- `deploy/Dockerfile.egress` (deploy-path mirror; secrets mount-only)
- `deploy/Dockerfile.twin-api` — curl healthcheck
- `deploy/docker-compose.app.yml` — egress + V1 + V2 + twin-api (embedded)
- `deploy/Caddyfile` + compose profile `tls` for HTTPS path

### Connections last-event age

- V1 metrics: `last_accepted_unix` on accept; exposed on `/healthz`
- twin-api: `V1_BASE_URL` (Docker-safe probes); `/v3/demo/status` returns
  `v1_last_event_age_secs`, `v1_accepted`, `v1_last_accepted_unix`
- Product UI Connections: “GitHub / ingest: last event Xm ago”

### OAuth / App scaffolding (blocked on human secrets)

- `deploy/oauth/slack-app-manifest.json`
- `deploy/oauth/github-app-manifest.yml`
- `deploy/oauth/README.md`
- UI disabled buttons for Install GitHub App / Connect Slack OAuth

## Still need from human

- Host + DNS (or confirm local-only)
- Slack OAuth client id/secret
- GitHub App credentials

## Follow-up in same session

- `/v3/onboarding/status` — server-driven checklist
- `/v3/oauth/slack/start`, `/v3/oauth/github/start` — 501 until env secrets
- Connections UI wizard steps + OAuth buttons with clear manual fallback

## Next code slices

1. Wire Slack OAuth **callback** → write bot token to egress vault only
2. Wire GitHub App install status into Connections  
3. Production-mode compose overlay (CRDB URLs on app services)
4. Public host + DOMAIN (human)
