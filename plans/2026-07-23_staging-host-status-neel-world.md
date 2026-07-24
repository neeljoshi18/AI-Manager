# Staging host — status.neel.world

**Date:** 2026-07-23  
**Status:** Host + DNS ready; app deploy not yet done on droplet  
**Repo deploy path:** monorepo `deploy/docker-compose.app.yml` (not a single binary on :8000)

## Confirmed facts (from founder)

| Field | Value |
|-------|--------|
| Provider | DigitalOcean |
| Region | blr1 (Bangalore) |
| Spec | 2 vCPU / 4GB / 80GB (`s-2vcpu-4gb`) |
| Image | DO 1-Click Docker (Ubuntu 22.04 + Docker Compose) |
| Public IPv4 | `206.189.129.31` |
| Private IP | `10.47.0.5` |
| Domain | `status.neel.world` |
| PUBLIC_BASE_URL | `https://status.neel.world` |
| TLS email | `neeljoshi18@gmail.com` |
| SSH | `ssh neel@206.189.129.31` (ed25519 key, no password, no root login) |
| DNS | A `status` → `206.189.129.31` (propagated) |
| UFW | 22 (limit), 80, 443 open |

## Product ports (AI Manager monorepo)

| Service | Internal | Role |
|---------|----------|------|
| twin-api | 18083 | Product UI `/app/`, lab `/demo/`, V3 API |
| V1 | 18080 | GitHub webhooks `/v1/...` |
| V2 | 18082 | Graph |
| egress | 18090 | Slack secrets inject (ADR-012) |
| Caddy | 80/443 | TLS + path routing |

**Not** a single Rust app on port 8000. Droplet scaffold under `~/app/` with one `app:8000` should be **replaced** by monorepo compose.

## Intended deploy layout on droplet

```
~/ai-manager/                    # git clone private repo
  deploy/docker-compose.app.yml
  deploy/Caddyfile               # DOMAIN=status.neel.world
  vertical-security/secrets/     # gitignored; create on host only
```

```bash
export DOMAIN=status.neel.world
# secrets file present first
docker compose -f deploy/docker-compose.app.yml --profile tls up -d --build
```

Caddy routes (repo `deploy/Caddyfile`):

- `/v1/*` → V1 (webhooks)
- everything else → twin-api (UI + V3)

## Security notes (ops)

- Prefer closing Docker API ports 2375/2376 on UFW if not required.
- Never commit `SLACK_BOT_TOKEN` / App private keys; host-only vault.
- Root domain `neel.world` stays on Vercel; only subdomain used.

## Still blocked for full product

- Slack OAuth client id/secret (optional if vault bot token already works)
- GitHub App pack for stable webhooks to `https://status.neel.world/v1/tenants/...`
- Deploy run on droplet (clone + compose) — next eng step once secrets or founder says “deploy now”

## Answers to scaffold questions

| Question | Answer |
|----------|--------|
| Framework | **Axum** (Rust), multi-binary monorepo |
| Bind | Services already bind `0.0.0.0:1808x` |
| Binary name | Multiple: `telemetry-ingestion`, `graph-api`, `twin-api`, `egress-proxy` |
| Source | Private GitHub `neeljoshi18/AI-Manager` |

## Deploy status (2026-07-23 later)

- Stack live at https://status.neel.world/app/
- Images built on Mac as **linux/amd64** and loaded to droplet (arm64 images will not run).
- Slack bot token in host vault only; OAuth client id/secret in deploy/.env.staging (gitignored).
- Caddy TLS issued for status.neel.world.
- **Droplet policy:** keep **always on** (no power-off between sessions).

## GitHub App (wired)

| Field | Value |
|-------|--------|
| App ID | 4374829 |
| Slug | `status-neel-world` |
| Installation | 148502033 on `neeljoshi18` |
| Webhook URL (must match App settings) | `https://status.neel.world/v1/tenants/ten_github/webhooks/github` |
| Secrets location | gitignored vault + `deploy/secrets/*.pem` on host only |

HMAC verified locally against V1 (`accepted` + last-event age).

## Always-on path (2026-07-24)

- `bridge` service in `deploy/docker-compose.app.yml`: V1 → V2 + twin upsert
- `scripts/github_live_bridge.py` uses ACL `bridge_reader` + `SLACK_USER_MAP`
- Twin-api still owns batched Slack DMs (`NOTIFY_INTERVAL_SECS=1800`)
- Bridge never DMs

