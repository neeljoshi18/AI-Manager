# Batch builds until one deploy (2026-07-31)

## Operating mode

- **Build + push to `main` continuously** (no droplet SSH from campus).
- **Single deploy later** when founder has hotspot **or** GitHub Actions secrets are set (`deploy/scripts/setup_ssh_via_https_port.md`).
- Do **not** ask for redeploy after every commit.

## Waiting on deploy (already on main)

- Commit/push digests + neighborhood soft-fail  
- ensure_users / membership seed before compile  
- Graph hide-demo + person collapse  
- Empty-draft upgrade for re-notify  
- GitHub Actions deploy workflow  
- Twin alias merge in compile (this batch)  
- Scheduler ensure V2 membership (this batch)  

## Post-deploy smoke (HTTPS only)

```bash
curl -sS https://status.neel.world/healthz
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/graph/ensure_users
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/team/compile \
  -H 'content-type: application/json' -d '{"force_notify":false,"allow_notify":true}'
```

Expect: ensure_users **200**, compile **ok** for both people, digests may have commit/repo items.
