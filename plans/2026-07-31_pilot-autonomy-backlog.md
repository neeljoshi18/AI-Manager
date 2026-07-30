# Pilot autonomy backlog (2026-07-31)

## Answered: lost history

**Yes.** Before V1 event journal + V2 graph snapshot + V3 twin state + V1 ACL identity map, embedded staging wiped everything on container rebuild. **Pre-durability graph history cannot be reconstructed from our stores** — only re-filled by:

1. New GitHub webhooks going forward (now journaled), or  
2. Full production CRDB/ClickHouse path later.

## Shipped this cycle

- V1 events + ACL identity persistence (stable `gu_*`)  
- V2 graph snapshot persistence  
- V3 twin persistence (earlier)  
- Intent seed (SHIP vs FREEZE + BLOCKED) for Team blockers  
- Twin prune one-per-Slack; graph hides duplicate Person labels  
- Deploy script no longer dies on sudo swap  

## Ops note (SSH / college Wi‑Fi)

- **College Wi‑Fi blocks SSH** to the droplet; **mobile hotspot works**.  
- Agent must **not SSH casually**. Only when a deploy/restart is required: stop and ask founder to switch to hotspot.  
- Prefer HTTPS APIs + git push for all other work.

## Open / next (do without asking unless blocked)

1. ~~After deploy: prune + multi-person verify~~ — team is 2 clean humans (2026-07-31)  
2. Graph UI: collapse same-label people + hide demo alice/bob by default  
3. Dual-person digests from real GH work (compile path + evidence)  
4. Partner install runbook: note Wi‑Fi/SSH for founder ops only  
5. Cockroach durability path when droplet can afford it  
6. Soft outreach package final check  

## ICP bar (recruitable pilot)

- [x] Anti-spam Notify v1  
- [x] Graph not mystery-empty after restart (persist)  
- [x] Blockers/intents demo surface  
- [x] Multi-person map (2 Slack, pruned)  
- [x] One clean person per human on Team API  
- [~] Graph UI collapse residual gu_* ghosts  
- [ ] Both humans real digests from GH activity  
- [ ] Partner install without founder SSH babysitting  
