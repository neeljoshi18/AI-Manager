# Session execution log — 2026-08-13

Ordered residual backlog from adequacy re-run + handoff. Mark status as work lands.

| # | Item | Status | Evidence |
|---|------|--------|----------|
| 1 | Re-run adequacy pack + write scores | **DONE** | `plans/packs/2026-08-12_ten_github_neeljoshi18.json` · `plans/2026-08-12_intent-adequacy-rerun.md` |
| 2 | Dogfood digests (no seed BLOCKS) | **DONE** | Fresh compile `blocker_count=0`, conf=medium; strip seed PR items in compiler |
| 3 | Slack channel claim path | **CODE DONE / INSTALL HUMAN** | `/v3/slack/events` captures channel+DM claims + commitments; needs bot in channel + Events URL (see below) |
| 4 | Follow-through on organic claims | **DONE** | API returns supported SHIP/FIX; folds ledger + slack claims |
| 5 | Role write-gates | **DONE** | `require_champion` on put_roles, put_tomorrow_focus, seed_graph_story, seed_intent_demo |
| 6 | CI → CiBlocked path | **DONE** | PR poller attaches commit status → `check_conclusion` / `ci_status` on PR nodes |
| 7 | Deploy + verify | **DONE** | `780d26a` deploy success; V1 recover_only after red; digests clean; roles 403/200 |
| 8 | Smoke stack + IST (2026-08-14) | **DONE** | v1/v2/v3/egress true · graph 327 · PR 13 · ledger_live github_pr 12 + explicit 2 · pulse `only_demo_seeds` · compile blockers=0 / medium / Lookback IST |
| 9 | Close remaining listed-UTC leaks | **CODE** | ledger `at`, graph `as_of`, team/profile digest times, roles/focus `updated_at`, engine/follow_through `as_of` → `+05:30`; cache `20260814a` |

## Slack install checklist (human — not blocked for code)

1. Slack app → Event Subscriptions → Request URL `https://status.neel.world/v3/slack/events`  
2. Subscribe bot events: `message.channels`, `message.groups`, `message.im`  
3. Invite bot to team channel (e.g. `C0APN754MQV`)  
4. Post: “blocked on security review” → appears in intent ledger / profile `slack_intent_claims`  
5. Post: “I’ll send the deck by Friday” → open commitment  

**Doctrine:** never silent private 1:1 wiretap; channel only when bot is a member.

## Role gate usage

- Pilot default: `champions: []` → everyone can write (open).  
- Multi-seat: `PUT /v3/tenants/ten_github/roles` with `{"champions":["neeljoshi18"],"actor_subject":"neeljoshi18"}`  
- Writes need `?actor=neeljoshi18` or `actor_subject` in body once list is non-empty.  
- Members still Approve / Don't send **own** digests via Slack map (not gated).  
