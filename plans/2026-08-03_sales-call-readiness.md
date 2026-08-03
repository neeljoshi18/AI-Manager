# Sales call readiness (target: day after tomorrow)

## Demo narrative (10 min)

1. **Today** — multi-person ready, blockers card (live vs demo seed labeled)
2. **Team** — 2 humans mapped; digests board with content/empty labels
3. **My status** — Approve / Edit / Don't send
4. **Graph** — people + commits/repos (hide demo); durability survives redeploy
5. **Anti-spam** — metrics suppress ≫ sent

## Must be green before soft outreach

| Check | How |
|-------|-----|
| HTTPS stack | healthz + demo status v1/v2/graph ok |
| Graph survives restart | durability files non-zero after twin/v2 recreate |
| Digests | ≥1 human non-empty; ideally 2 |
| multi_person_ready | true |
| Partner docs | one-pager + install runbook |

## Fast deploys (after this commit)

```bash
# On hotspot / Actions with BuildKit cache: only rebuild changed services
./deploy/scripts/deploy_fast.sh          # auto
./deploy/scripts/deploy_fast.sh twin-api # static/API only (~few min after cache warm)
./deploy/scripts/deploy_fast.sh --no-build  # rsync + restart only

# Campus always:
git push origin main   # Actions deploys
gh workflow run deploy-staging.yml -f skip_build=true  # no cargo rebuild
```

## Founder actions (minimal)

- Keep droplet on for demos
- Optional: second person push a PR/commit as paneerjeera for dual digests
- Soft outreach after pilot_readiness soft_outreach_ready OR honest solo+map demo
