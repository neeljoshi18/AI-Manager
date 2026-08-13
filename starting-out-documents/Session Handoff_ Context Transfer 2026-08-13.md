# Session Handoff — Context Transfer 2026-08-13

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Cache-bust:** hard-refresh → `app.js?v=20260813a` + styles  
**HEAD (at handoff):** `6232ba0` — IST timestamps · prior: `780d26a` residual backlog · `34fde5d` organic PR poller  
**Purpose:** Full arc after Simple Home polish, organic PR intents, residual backlog close, **IST display conversion**.  
**Do not auto-compact** — handoff + new chat when context is high.

---

## 0. What shipped this arc (high level)

| Area | Status |
|------|--------|
| Simple Home deep board (promises, focus, filters, digest preview) | **Live** |
| Plain-English polish + `promiser_display` (no raw `gu_*` in actions) | **Live** |
| Organic **PR poller** + labels/body on V1 normalize | **Live** (13 PRs, 12 `github_pr` claims) |
| Digest seed strip (BLOCKS + story-1 PR items) | **Live** (fresh compile `blockers=0`) |
| Pulse honest empty (`empty_reason=only_demo_seeds`) | **Live** |
| Adequacy pack re-run + score doc | **Done** |
| Role write-gates (champion `?actor=`) | **Live** (403/200 verified) |
| Follow-through on organic claims | **Live** |
| PR CI status → CiBlocked path | **Code live** |
| **All listed timestamps → IST (+05:30)** | **Live** `20260813a` |
| Neon twin + graph mirror | Still continuous |
| Simple vs Technical | Presentation only (all features both modes) |

### Doctrine (unchanged)

- Actions deploy not hotspot; gas; no Linear-as-core/training; vault for tokens; **no LOC rankings**  
- Chat = delivery; GitHub = work  
- ADR-012 vault only  
- Do **not** break Slack default  
- Never private 1:1 wiretap as product default  

---

## 1. Paste prompt (copy entire block into next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-13.md  ← THIS FILE
2. plans/2026-08-13_session-execution-log.md
3. plans/2026-08-12_intent-adequacy-rerun.md
4. plans/2026-08-12_priority-and-organic-pr-wedge.md
5. plans/2026-08-06_intent-adequacy-experiment.md
6. plans/2026-08-07_intent-engine-design.md
7. plans/2026-08-07_how-we-classify-intent-simple.md

Doctrine: Actions deploy not hotspot; gas; no Linear-as-core/training; vault for tokens; no LOC rankings.
Chat = delivery; GitHub = work. ADR-012 vault only. Do not break Slack default.

STAGING: https://status.neel.world/app/
Hard-refresh (Cmd+Shift+R) so app.js?v=20260813a + styles load.

UX CONTRACT:
- Simple vs Technical is PRESENTATION ONLY. Every feature both modes.
- Simple = #ck-visual-home visual org story. Technical = #ck-ops-stack operator console.
- NOT feature-gating Graph/Lab/etc.

TIMEZONE CONTRACT (2026-08-13):
- All listed product timestamps are IST (UTC+05:30 exact).
- twin_core::time_ist — FixedOffset; never invent times.
- Digests lookback, insights heat, pulse/profile as_of, commitments, app fmtIst().
- Storage instants still UTC under the hood; listed RFC3339 uses +05:30.

Intent stack:
- GET …/intent/engine, …/intent/ledger, …/intent/insights
- POST …/intent/claims, supersede
- Commitments: GET/POST …/commitments, done/dismiss, digest, export_linear
- People: directory, profile, follow_through
- Bridge: commit + PR poller (scripts/github_live_bridge.py)

LIVE TRUTH (verify with curl after hard-refresh):
- PR nodes ≫1; ledger_live github_pr claims; pulse empty_reason only_demo_seeds when no live friction
- Fresh digests: blocker_count=0; conf medium; no "Blocked via BLOCKS" from seeds
- Roles: champions may be set; writes need ?actor= when champions[] non-empty

NEXT (priority):
1. Smoke stack + IST surfaces (insights as_of +05:30, digest Lookback IST).
2. Human-only: Slack Events URL + invite bot to channel for ambient claims.
3. Optional env: COMMITMENT_DIGEST_CHANNEL, COMMITMENT_DIGEST_HOUR_IST, LINEAR_* .
4. Organic live conflict when dual-owner real PR friction appears (do not seed as product truth).
5. V1 may flip red after deploy — Actions recover_only=true.

Campus SSH often blocked — Actions recover_only. No secrets in chat.
Handoff when context high — no auto-compact.
Start: hard-refresh app + curl insights/dev, pulse, intent/engine, team/compile preview.
```

---

## 2. Attach list (upload / @-mention these paths)

### Required (start here)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-13.md` ← **this file**  
2. `plans/2026-08-13_session-execution-log.md`  
3. `plans/2026-08-12_intent-adequacy-rerun.md`  
4. `plans/2026-08-12_priority-and-organic-pr-wedge.md`  
5. `plans/packs/2026-08-12_ten_github_neeljoshi18.json`  
6. `plans/2026-08-06_intent-adequacy-experiment.md`  

### Product / design context

7. `plans/2026-08-07_intent-engine-design.md`  
8. `plans/2026-08-07_how-we-classify-intent-simple.md`  
9. `plans/intent-research.md`  
10. `plans/2026-08-08_next-upgrades-shipped.md`  
11. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-08.md` (prior UX contract)  
12. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-12.md` (PR wedge)  

### Code (touch surfaces)

13. `vertical-3/app-static/app.js`  
14. `vertical-3/app-static/index.html`  
15. `vertical-3/app-static/styles.css`  
16. `vertical-3/crates/twin-core/src/time_ist.rs`  
17. `vertical-3/crates/twin-core/src/ledger_text.rs`  
18. `vertical-3/crates/twin-api/src/main.rs`  
19. `vertical-3/crates/twin-api/src/commitments.rs`  
20. `vertical-3/crates/twin-api/src/intent_engine.rs`  
21. `vertical-3/crates/twin-compiler/src/lib.rs`  
22. `vertical-3/crates/twin-delivery/src/worker.rs`  
23. `scripts/github_live_bridge.py`  

### Ops (if Neon / deploy)

24. `plans/observe-neon.md`  
25. `.github/workflows/deploy-staging.yml`  

---

## 3. Absolute paths (Finder / attach paste)

Repo root: `/Users/neelvaanjoshi/Desktop/ai-manager`

```
/Users/neelvaanjoshi/Desktop/ai-manager/starting-out-documents/Session Handoff_ Context Transfer 2026-08-13.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-13_session-execution-log.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-12_intent-adequacy-rerun.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-12_priority-and-organic-pr-wedge.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/packs/2026-08-12_ten_github_neeljoshi18.json
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-06_intent-adequacy-experiment.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-07_intent-engine-design.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-07_how-we-classify-intent-simple.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/intent-research.md
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/app.js
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/index.html
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/styles.css
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-core/src/time_ist.rs
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-core/src/ledger_text.rs
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-api/src/main.rs
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-api/src/commitments.rs
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-compiler/src/lib.rs
/Users/neelvaanjoshi/Desktop/ai-manager/scripts/github_live_bridge.py
```

**Minimal attach set (if context limited):** handoff 08-13 + execution log + adequacy re-run + `time_ist.rs` + `app.js` + `github_live_bridge.py` + `twin-compiler/lib.rs`.

---

## 4. Live snapshot (at handoff — re-verify)

```
Staging app:     ?v=20260813a
Stack:           v1/v2/v3/egress true · graph_nodes ~326
Graph by_type:   Commit ~254 · PullRequest 13 · Intent 20 · Person 11 · Repo 14
ledger_live:     github_pr 12 + explicit 2
as_of sample:    …+05:30 · timezone Asia/Kolkata
insight:         Most active hour (IST): 18:00 …
Observe:         external_db + graph_mirror true
```

### Ops cheatsheet

```bash
# Stack + IST
curl -s https://status.neel.world/v3/demo/status | jq '{v1,v2,v3,egress,graph_nodes}'
curl -s https://status.neel.world/v3/tenants/ten_github/insights/dev | jq '{as_of,timezone,insight:.activity.insight,by_type:.graph.by_type}'
curl -s https://status.neel.world/v3/tenants/ten_github/pulse | jq '{as_of,timezone,conflicts:(.conflicts|{count,demo_count,empty_reason})}'
curl -s https://status.neel.world/v3/tenants/ten_github/intent/engine | jq '.ledger_live'
curl -s -X POST https://status.neel.world/v3/tenants/ten_github/team/compile \
  -H 'content-type: application/json' -d '{"allow_notify":false}' \
  | jq '.results[]|{name:.display_name,blockers:.blocker_count,conf:.confidence,preview:(.preview|.[0:120])}'

# Roles gate
curl -s 'https://status.neel.world/v3/tenants/ten_github/roles?actor=neeljoshi18' | jq .

# Recover V1: Actions → Deploy staging → recover_only=true
```

**Env optional:**  
`COMMITMENT_DIGEST_CHANNEL`, `COMMITMENT_DIGEST_HOUR_IST` (preferred) / `COMMITMENT_DIGEST_HOUR_UTC`, `LINEAR_API_KEY`, `LINEAR_TEAM_ID`, `OBSERVE_DATABASE_URL` (GH secret).

---

## 5. Product truth

| Layer | Truth |
|-------|--------|
| Trajectory | Strong (commits + multi-repo poller) |
| Organic claims | **Unlocked** — github_pr + explicit; not seed-only |
| Conflicts live | Often empty with **honest** `only_demo_seeds` |
| Digests | Fresh compiles clean of seed BLOCKS/story-1; **old published** drafts may still show historical seed text |
| Simple/Technical | Presentation only |
| Time listed | **IST only** on product surfaces |

**Never:** LOC rankings · private 1:1 wiretap · sell demo SHIP/FREEZE as org politics · Linear as core product.

---

## 6. Residual (next session)

| Pri | Item | Who |
|-----|------|-----|
| P1 | Slack Events URL + bot in team channel (ambient claims) | **Human** |
| P1 | Smoke after any deploy (V1 red, IST still +05:30) | Agent |
| P2 | Optional morning digest channel / Linear keys | Human env |
| P2 | First organic **live** dual-owner conflict card | Data / work |
| P2 | Recorded 5-min never-fail demo | Human |
| P3 | Tickets as claims; multi-tenant volumes | Later |

---

## 7. Explicit session boundary

| In next session | Do not |
|-----------------|--------|
| Smoke + continue residual P1 | Restart cockpit/sales PDFs from zero |
| Wire Slack Events if human ready | Auto-compact this arc |
| Optional digest/Linear env | Put secrets in chat |
| Keep IST display contract | Revert listed times to UTC |
| | Pretend demo intents are live intelligence |

---

## 8. Document control

| Field | Value |
|-------|--------|
| Prior handoffs | `…2026-08-12.md`, `…2026-08-08.md` |
| Adequacy baseline | `plans/2026-08-06_intent-adequacy-experiment.md` |
| Adequacy re-run | `plans/2026-08-12_intent-adequacy-rerun.md` |
| Execution log | `plans/2026-08-13_session-execution-log.md` |
| IST module | `vertical-3/crates/twin-core/src/time_ist.rs` |
| Compaction | **Handoff instead of auto-compact** |
