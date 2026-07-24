# Plan: M6 multi-member beta path (implementation slice)

**Date:** 2026-07-24  
**Status:** Implemented (code + docs); Linear connector + channel ingest deferred  
**Ground truth:** Session Handoff 2026-07-23, ADR-016, `plans/2026-07-24_onprem-model-and-agents.md`

---

## Goal

Beta-ready path **without** local model training:

- Multi-person Slack map (2+ humans)
- Intent classification v0 (rules → graph)
- Conflict detector v0 + UI surface
- Thin monitors (pulse cache, metrics stubs)
- Design-partner playbook

**Out of scope (later M6 / M6.5):** Linear productized connector, Slack channel short-text ingest, Model Router / Ollama (M7).

---

## What shipped

| Item | Where |
|------|--------|
| Intent rules v0 (SHIP/BLOCKED/FIX/…) | `vertical-2/crates/graph-core/src/intent.rs` |
| Attach on PR/issue project | `project.rs` → Intent node + CLAIMS + ABOUT |
| Conflicts API | `GET /v2/tenants/{t}/conflicts`, `/intents` |
| Team map API | `GET/POST /v3/tenants/{t}/team`, `…/team/members` |
| Pulse + thin monitor | `GET /v3/tenants/{t}/pulse`; scheduler tick |
| Metrics stubs | `/metrics`: DMs, veto rate, empty windows, conflict hits |
| Product UI | Team nav; Today conflicts; Settings metrics |
| Bridge multi-person | Merges twin team `bridge_slack_map` + `SLACK_USER_MAP` |
| Partner docs | `starting-out-documents/Design Partner_*.md` |

---

## Verify

```bash
cd vertical-2 && cargo test -p graph-core intent && cargo run -p graph-verify
cd vertical-3 && cargo build -p twin-api && cargo run -p twin-verify
# UI: /app/ → Team map ≥2 humans; Today shows conflicts after PR titles/labels
```

---

## Next M6 slices

1. Seed second human on staging (`status.neel.world`) via Team UI  
2. Linear webhook → V1 → same intent path  
3. Optional: include conflict summary in **batched** team channel (not 1:1 spam)  
4. Learning-window metrics dashboard polish  
