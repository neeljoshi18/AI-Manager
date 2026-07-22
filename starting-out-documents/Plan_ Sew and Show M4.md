# Plan: Sew & Show → Milestone M4

**Status:** In progress (autonomous execution)  
**Date:** 2026-07-22  

Full operational plan (deliverables, Slack checklist, context handoff, M5–M7) lives in the session plan file and is summarized here for git continuity.

## Goal

Make the product **visible and sewn**:

```
Event → V1 → V2 → V3 ledger → Slack DM (egress) → veto/edit/publish
```

## Done when (M4 acceptance)

- [x] Platform sew script `scripts/platform_sew.sh` (embedded + live modes)
- [x] Demo console at `http://127.0.0.1:18083/demo/`
- [ ] Real Slack DM via egress (needs founder bot token)
- [ ] Human demo script (this folder)
- [ ] Session handoff updated

## Non-goals

No V4, no Buzz/Centaur, no search index, no multi-tenant SaaS yet (M7).

## Commands

```bash
cd vertical-3 && RUNTIME_MODE=embedded cargo run -p twin-api
open http://127.0.0.1:18083/demo/
./scripts/platform_sew.sh
cd vertical-3 && cargo run -p twin-verify
```
