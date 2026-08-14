# Session Handoff — Context Transfer 2026-08-14

**Repo:** `neeljoshi18/AI-Manager` · branch **`main`**  
**Staging:** https://status.neel.world/app/  
**Cache-bust:** hard-refresh → `app.js?v=20260814d` + styles  
**HEAD (at handoff):** will be the commit that ships this file + `person:gu_*` insight hygiene  
**Purpose:** Continue after IST leak-close, Slack Events install surface, circled-eye id hygiene, and Worth-watching owner-name fix.  
**Do not auto-compact** — handoff + new chat when context is high.

**Next session start rule:** Read the files. Do **not** implement, deploy, or “continue residual” until the user pastes a separate UI prompt. The next arc is **UI polish**.

---

## 0. What shipped this arc (2026-08-14)

| Area | Status |
|------|--------|
| Smoke + IST leftover listed timestamps (`+05:30`) | **Live** `8750165` · `72e41d3` |
| Slack Events URL on Connect + OAuth channel-history scopes | **Live** `81183a6` · cache `20260814b` |
| Circled-eye hide for opaque ids (hashes, `dft_`/`led_`/`gu_*`, Slack U/C ids) | **Live** `f33a9e8` · cache `20260814c` |
| Worth watching / intent insights: never print `person:gu_…` | **This ship** · cache `20260814d` |
| V1 | Often red right after compose deploy; `recover_only=true` usually restores. `accepted` resets to 0 on recreate (in-memory). Graph/poller still fill. |

### Doctrine (unchanged)

- Actions deploy not hotspot; gas; no Linear-as-core/training; vault for tokens; **no LOC rankings**
- Chat = delivery; GitHub = work
- ADR-012 vault only
- Do **not** break Slack default
- Never private 1:1 wiretap as product default
- Simple vs Technical = **presentation only** (not feature-gating)
- All listed product timestamps = IST (`+05:30`); storage instants UTC

### UX contract

- Simple = `#ck-visual-home` visual org story
- Technical = `#ck-ops-stack` operator console
- **Both** must read as words first. Machine ids go behind the circled eye (hover reveal, click copy)
- Feedback from sales calls: raw `person:gu_…` / commit hashes are intimidating even to technical people — treat any leftover dump as a bug

### Timezone contract

- `twin_core::time_ist` — FixedOffset UTC+05:30; never invent times
- Digests lookback, insights heat, pulse/profile as_of, commitments, app `fmtIst()`
- Listed RFC3339 uses `+05:30`

---

## 1. Paste prompt (copy entire block into next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager, branch main.

READ FIRST (in order). Do not implement, edit, deploy, or start residual work until I send a separate UI prompt. This session is for UI polish; wait for that prompt after you have loaded context.

1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-14.md  ← THIS FILE
2. plans/2026-08-13_session-execution-log.md
3. plans/2026-08-12_intent-adequacy-rerun.md
4. plans/2026-08-12_priority-and-organic-pr-wedge.md
5. plans/2026-08-06_intent-adequacy-experiment.md
6. plans/2026-08-07_intent-engine-design.md
7. plans/2026-08-07_how-we-classify-intent-simple.md
8. vertical-3/app-static/app.js
9. vertical-3/app-static/index.html
10. vertical-3/app-static/styles.css
11. vertical-3/crates/twin-api/src/commitments.rs
12. vertical-3/crates/twin-api/src/main.rs

Doctrine: Actions deploy not hotspot; gas; no Linear-as-core/training; vault for tokens; no LOC rankings.
Chat = delivery; GitHub = work. ADR-012 vault only. Do not break Slack default.

STAGING: https://status.neel.world/app/
Hard-refresh (Cmd+Shift+R) so app.js?v=20260814d + styles load.

UX CONTRACT:
- Simple vs Technical is PRESENTATION ONLY. Every feature both modes.
- Simple = #ck-visual-home visual org story. Technical = #ck-ops-stack operator console.
- NOT feature-gating Graph/Lab/etc.
- First view must be understandable words in BOTH modes. Opaque ids (person:gu_*, dft_*, led_*, commit SHAs, Slack U/C ids) sit behind a circled eye (.id-eye / prettyRef / scrubTextHtml). Hover reveals; click copies.
- Sales-call feedback: raw graph ids in sentences (e.g. “person:gu_ec3c… is aiming to ship”) are intimidating. Treat leftover dumps as bugs.

TIMEZONE CONTRACT:
- All listed product timestamps are IST (UTC+05:30 exact).
- twin_core::time_ist — FixedOffset; never invent times.
- Storage instants still UTC; listed RFC3339 uses +05:30.

Intent stack:
- GET …/intent/engine, …/intent/ledger, …/intent/insights
- POST …/intent/claims, supersede
- Commitments: GET/POST …/commitments, done/dismiss, digest, export_linear
- People: directory, profile, follow_through
- Insights copy: commitments::build_plain_insights + human_owner_label (never person:gu_* in Worth watching)
- Bridge: commit + PR poller (scripts/github_live_bridge.py)

LIVE TRUTH (do not curl/smoke unless I ask after the UI prompt):
- PR nodes ≫1; ledger_live github_pr + explicit; pulse empty_reason only_demo_seeds when no live friction
- Fresh digests: blocker_count=0; conf medium; no seed “Blocked via BLOCKS”
- Roles: champions may be set; writes need ?actor= when champions[] non-empty
- Slack Events URL: https://status.neel.world/v3/slack/events (Connect copies it). Human: paste in Slack app + invite bot + reconnect Slack for channel-history scopes.

HUMAN / ENV (not agent-blocked):
- Slack Events Request URL + invite bot to C0APN754MQV
- Reconnect Slack if token predates channels:history
- Optional: COMMITMENT_DIGEST_CHANNEL, COMMITMENT_DIGEST_HOUR_IST, LINEAR_*

THIS SESSION FOCUS: UI polish (I will send a detailed UI prompt next). After reading, reply with a short “context loaded” only — no code changes, no deploy, no residual engineering until that prompt.

Campus SSH often blocked — Actions recover_only=true if V1 is red after a later deploy. No secrets in chat.
Handoff when context high — no auto-compact.
```

---

## 2. Attach list (absolute paths)

Repo root: `/Users/neelvaanjoshi/Desktop/ai-manager`

### Required

```
/Users/neelvaanjoshi/Desktop/ai-manager/starting-out-documents/Session Handoff_ Context Transfer 2026-08-14.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-13_session-execution-log.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-12_intent-adequacy-rerun.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-12_priority-and-organic-pr-wedge.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-06_intent-adequacy-experiment.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-07_intent-engine-design.md
/Users/neelvaanjoshi/Desktop/ai-manager/plans/2026-08-07_how-we-classify-intent-simple.md
```

### UI (next-session focus — attach these)

```
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/app.js
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/index.html
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/app-static/styles.css
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-api/src/commitments.rs
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-api/src/main.rs
```

### Optional if UI work touches graph / time / Slack install

```
/Users/neelvaanjoshi/Desktop/ai-manager/vertical-3/crates/twin-core/src/time_ist.rs
/Users/neelvaanjoshi/Desktop/ai-manager/deploy/oauth/README.md
/Users/neelvaanjoshi/Desktop/ai-manager/deploy/oauth/slack-app-manifest.json
/Users/neelvaanjoshi/Desktop/ai-manager/scripts/github_live_bridge.py
```

**Minimal attach set:** this handoff + execution log + `app.js` + `index.html` + `styles.css` + `commitments.rs`.

---

## 3. Live snapshot (re-verify after hard-refresh)

```
Staging app:     ?v=20260814d
Stack:           v2/v3/egress typically true · V1 may be red after deploy (recover_only)
Graph:           Commit 250+ · PullRequest 13 · Intent 20 · Person 11 · Repo 14
ledger_live:     github_pr + explicit (not seed-only)
pulse:           empty_reason=only_demo_seeds when no live dual-owner friction
insights:        worth_watching uses display names / “Someone” — never person:gu_*
id hygiene:      .id-eye in app.js (prettyRef, scrubTextHtml)
```

### Ops

```bash
# After a code deploy, if v1 is false:
# Actions → Deploy staging → recover_only=true
curl -s https://status.neel.world/v3/demo/status | jq '{v1,v2,v3,egress,graph_nodes}'
curl -s https://status.neel.world/v3/tenants/ten_github/intent/insights | jq '.worth_watching[].text'
```

**Never:** LOC rankings · private 1:1 wiretap · sell demo SHIP/FREEZE as org politics · Linear as core · print `person:gu_*` in champion copy.

---

## 4. Residual

| Pri | Item | Who |
|-----|------|-----|
| **UI** | Next session: polish (user will send a dedicated prompt). Hunt leftover intimidating ids/jargon. | Next agent — **wait for prompt** |
| P1 | Slack Events URL + invite bot + reconnect for channel-history scopes | Human |
| P1 | V1 red after deploy → Actions `recover_only=true` | Agent only after a deploy |
| P2 | Morning digest / Linear env | Human |
| P2 | First organic live dual-owner conflict | Data — do not seed |

---

## 5. Document control

| Field | Value |
|-------|--------|
| Prior handoffs | `…2026-08-13.md`, `…2026-08-12.md`, `…2026-08-08.md` |
| Execution log | `plans/2026-08-13_session-execution-log.md` |
| Eye helper | `prettyRef` / `idEye` / `scrubTextHtml` in `app-static/app.js` |
| Insight owner names | `commitments::human_owner_label` |
| Compaction | **Handoff instead of auto-compact** |
| Next session | **Load only. UI focus. Wait for user’s UI prompt.** |
