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
2. ~~Graph UI: collapse same-label people + hide demo alice/bob~~ (on main; needs deploy)  
3. ~~Digests include commits/pushes~~ (on main; needs deploy) — was empty for pure git activity  
4. ~~Neighborhood 404 soft-fail~~ so multi-person compile continues  
5. Dual-person digests with non-empty items after deploy + ensure_users — **code path airtight** (24h lookback, multi-alias, tests); live still needs deploy + paneerjeera GH edges  
6. Cockroach durability path when droplet can afford it  
7. ~~Soft outreach package final check~~ — `plans/2026-08-01_soft-outreach-checklist.md`  
8. ~~Hide demo seed server-side on graph~~ (2026-08-01)  
9. Live A2 after one deploy (blocked on hotspot/Actions)

## ICP bar (recruitable pilot)

- [x] Anti-spam Notify v1  
- [x] Graph not mystery-empty after restart (persist)  
- [x] Blockers/intents demo surface  
- [x] Multi-person map (2 Slack, pruned)  
- [x] One clean person per human on Team API  
- [~] Graph UI collapse residual gu_* ghosts  
- [ ] Both humans real digests from GH activity  
- [ ] Partner install without founder SSH babysitting  
