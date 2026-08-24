# DigitalOcean VPS — paused (2026-08-15)

The **DigitalOcean droplet is powered off** (founder closed it).  
`https://status.neel.world/app/` will not load until a VPS is on again and DNS points at it.

**Do not delete** the droplet deploy path. It is kept in-repo, currently disabled.

## Active path (laptop)

```bash
cd /Users/neelvaanjoshi/Desktop/ai-manager
./scripts/dev_up.sh
# Product UI:
open http://127.0.0.1:18083/app/
# Stop:
./scripts/dev_down.sh
```

This is the same local stack we used **before** the droplet (`plans/2026-07-23_wake-laptop-runbook.md`).

| Service | Port |
|---------|------|
| V1 ingest | `:18080` |
| V2 graph | `:18082` |
| V3 twin-api + `/app/` | `:18083` |
| Egress (if vault present) | `:18090` |

Optional GitHub webhooks while on a laptop: `ngrok http 18080` (see `starting-out-documents/GitHub Webhook Setup_ Local.md`).

## What was disabled (not deleted)

| Path | How it’s paused |
|------|-----------------|
| `.github/workflows/deploy-staging.yml` | `on.push` commented out; job `if: false` |
| `.github/workflows/recover-staging.yml` | job `if: false` |
| `deploy/scripts/sync_and_deploy_staging.sh` | early `exit 1` after pause banner |
| `deploy/scripts/deploy_when_ssh.sh` | same |
| `deploy/scripts/deploy_from_ci_or_hotspot.sh` | same |
| `deploy/scripts/ssh_staging.sh` | same |
| `deploy/scripts/deploy_fast.sh` | same |

Droplet address that **used to** serve staging: `neel@206.189.129.31` · domain `status.neel.world`.

## Restore VPS later

1. Power on a droplet (or create a new one) and put `status.neel.world` DNS back on it.  
2. Remove the pause banners / `if: false` / commented `on.push`.  
3. Re-run **Deploy staging** from Actions (or `./deploy/scripts/sync_and_deploy_staging.sh` from a network that can SSH).
