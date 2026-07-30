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

## Open / next (do without asking unless blocked)

1. After deploy: prune + reseed intent + verify single neeljoshi18 + paneerjeera  
2. Merge bridge twin upsert onto existing seed twin when login alias matches (avoid second twin on first real gu)  
3. Team UI: prune button or auto-prune on Team refresh  
4. Remove demo alice/bob from default graph view filters (or tag seed)  
5. Cockroach durability path when droplet can afford it  
6. Real dual-account PR digests 3-day dry-run  
7. Partner outreach package final check  

## ICP bar (recruitable pilot)

- [x] Anti-spam Notify v1  
- [x] Graph not mystery-empty after restart (persist)  
- [x] Blockers/intents demo surface  
- [x] Multi-person map  
- [ ] One clean person per human on Graph (in progress this deploy)  
- [ ] Both humans real digests from GH activity  
- [ ] Partner install without founder SSH babysitting  
