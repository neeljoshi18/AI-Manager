# Plan: Confidence first, airtight product, recruitable pilot

**Date:** 2026-07-27 (updated 2026-07-30)  
**Status:** Direction approved; Notify Policy v1 **shipped + live-verified**; A3 recovery + A5 runbook shipped 2026-07-30; multi-person digests still open  
**Ground truth handoff:** `starting-out-documents/Session Handoff_ Context Transfer 2026-07-27.md`

---

## Build standard (forever)

**Airtight or don’t ship.** End-to-end, observable, non-spammy, stranger-readable. No half-surfaces.

Lessons from slop (Graph 0/0, spam DMs, unproven multi-person, jargon “veto”, shabby pitch): see handoff §0.

---

## Confidence targets

| Level | Requirement |
|-------|-------------|
| ~30% | Solo demo (past) |
| **&gt;50% soft outreach** | A1–A7 + 3-day dry-run with 2 people |
| **&gt;70% confident pilots** | One external partner finishes 10–14d |
| &gt;85% | Second connector or self-serve |

---

## A-list (must close for pilot confidence)

| ID | Item | Status |
|----|------|--------|
| A1 | Notify Policy v1 (change-only + daily cap) | **Live-verified 2026-07-30** — staging `/metrics`: 168 compiles, 2 DMs sent, **166 suppressed**, `notify_policy: v1_change_only_daily_cap` |
| A2 | Multi-person digests proven | **Plumbing airtight 2026-07-30** — map + seed + Team digests board + `POST …/team/compile` + persist; p2 force-DM sent. **Live real-PR digests for both GH logins** still needs activity / post-deploy seed |
| A3 | Graph durability / no mystery empty | **Hardened 2026-07-30** — bridge recovery + twin-api **embedded state file** (`TWIN_EMBEDDED_STATE_PATH`) so team maps survive restarts |
| A4 | Approve / Edit / Don’t send UX | **Shipped** + 2026-07-30 evidence/empty-draft polish |
| A5 | Partner install runbook | **Shipped** — `starting-out-documents/Design Partner_ Install Runbook.md` |
| A6 | Empty/wrong draft UX | **Improved** — evidence on items; empty banner; no-DM copy |
| A7 | Pilot packaging aligned to real product | **Aligned** — one-pager + learning-window playbook (Approve language, Notify v1, install link) |

---

## B-list (later)

Linear · Slack channels · multi-tenant · learning-window machine · Model Router · rich conflict Slack · browser  

---

## Positioning (after airtight loop)

**Primary:** “Status that writes itself from your PRs — you approve before anyone sees it.”  
**Alt:** “Kill the standup. Keep the signal.”

---

## Sprint order

1. ~~Verify anti-spam live~~ **done (metrics 2026-07-30)**  
2. Multi-person airtight — **needs founder: 2nd human map fields**  
3. ~~Pilot package docs~~ **done (runbook + align)** — soft outreach only after A2 green  
4. One full connector only if demanded  

**Not now:** local model training (ADR-016).

---

## Notify Policy v1 (reference)

- Fingerprint ledger items + blockers (not window alone)  
- Unchanged → no DM  
- Max 1 status DM / person / UTC day (unless new blocker / force)  
- Metrics: `twin_dms_suppressed_total`  

---

## Live staging snapshot (2026-07-30 session)

| Probe | Result |
|-------|--------|
| `https://status.neel.world/healthz` | twin-api ok |
| `/metrics` | suppress 166 · sent 2 · compiles 168 |
| `/v3/tenants/ten_github/graph` | nodes 28 · edges 101 · v2_up |
| `/v3/tenants/ten_github/team` | person_count 1 · multi_person_ready **false** |
| SSH deploy from this agent host | timed out — push code; founder deploy when SSH available |

---

## Need from founder for A2

For second human (display_name, slack_user_id `U…`, github_login, github_numeric_id, tenant `ten_github`) — Team map + optional `SLACK_USER_MAP` entry. Then prove both digests + Graph shows 2 people.

---

## New session prompt

See handoff §15 — paste with attached file list in handoff §12.
