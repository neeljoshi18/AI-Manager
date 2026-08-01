# Soft-outreach readiness checklist (2026-08-01)

**Purpose:** Go/no-go before messaging strangers. Does **not** require a live deploy to maintain this checklist — but **A2 live dual digests** does require one deploy + real second-human GH activity.

**Staging:** https://status.neel.world/app/  
**Handoff:** `starting-out-documents/Session Handoff_ Context Transfer 2026-07-31.md`

---

## Can ship on main without hotspot (done / do now)

| Item | Status |
|------|--------|
| Notify Policy v1 (change-only + daily cap) | **Done** (live-verified earlier) |
| Digests: commits/pushes/PRs + multi-identity gu_* merge | **Done on main** |
| 24h rolling lookback + open-PR keep | **Done on main** |
| Team compile proof fields (`with_items`, `empty_reason`, kinds) | **Done on main** |
| Graph person collapse | **Done on main** |
| Graph **server-side** hide demo alice/bob | **Done on main** (this batch) |
| Pulse demotes intent_demo conflicts | **Done on main** (this batch) |
| Partner one-pager / runbook / learning window | **Aligned** (24h, Approve language, no-SSH ops) |
| Campus deploy path (Actions workflow) | **Code done** — needs founder `STAGING_*` secrets once |

## Blocked on founder deploy / hotspot

| Item | Why |
|------|-----|
| Staging runs latest `main` | Campus SSH blocked; Actions secrets optional |
| Live A2: person1 non-empty digest | Needs new image + compile after deploy |
| Live A2: person2 non-empty digest | Needs paneerjeera (or other) **real GH edges** |
| Partner install without SSH babysitting | Actions secrets + one green workflow |

## Soft outreach A1–A7 bar

| ID | Bar | Code path | Live staging |
|----|-----|-----------|--------------|
| A1 | Anti-spam | Done | Done (historical metrics) |
| A2 | Dual digests real content | Plumbing + tests | **Pending deploy + activity** |
| A3 | Graph durability | Done | Verify after deploy |
| A4 | Approve / Edit / Don't send | Done | Pending deploy for latest UI |
| A5 | Install runbook | Done | — |
| A6 | Empty draft UX | Done | Pending deploy |
| A7 | Packaging | Done | — |

**Soft outreach:** only after A2 **live** green (both humans ≥1 non-empty digest from real work, not only intent_demo).

## Efficient while waiting for hotspot

1. Keep batching on `main` (this mode).  
2. Do **not** spam SSH.  
3. Optional cost control: if droplet is idle for days and you accept downtime, power off the DO droplet until hotspot day — product work continues via git; staging smoke waits. (Do **not** turn off unless you choose to; agent will not power-off for you.)  
4. One-time when hotspot ready: set Actions secrets → one deploy → smoke in handoff §3 → then soft outreach.

## Explicitly not now

Linear connector · local model training (ADR-016) · full Cockroach multi-service · self-serve multi-tenant.

---

## Pitch (copy/paste)

> Status that writes itself from your PRs and pushes — you approve before anyone sees it.  
> Two weeks of shadow digests. Kill one standup if Don't-send rate is healthy. We never silent-read private 1:1s.
