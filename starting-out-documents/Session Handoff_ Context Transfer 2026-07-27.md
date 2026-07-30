# Session Handoff — Context Transfer

**Date:** 2026-07-27  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager` (branch `main`)  
**Purpose:** Full handoff for a **new chat**. Prefer this file + listed ground-truth docs over compacted history.  
**Supersedes for “what to do next”:** older handoff `Session Handoff_ Context Transfer 2026-07-23.md` (keep for ports/commands history).

---

## 0. Ground rule — **never half-ass** (founder mandate 2026-07-26)

**When we ship a capability, it is airtight:**

- End-to-end (ingest → graph → UI/Slack if claimed)  
- Observable (health, metrics, clear empty/error states)  
- Non-spammy (developer-first Slack)  
- Understandable to a stranger (no TAS jargon as the only UI language)  
- Documented for pilot use  

**Do not** ship “exists in code / nav item” that fails under real use (empty Graph 0/0, Team map with no multi-person digests, OAuth stubs sold as self-serve, same open PR DMing every 30 minutes).

**Slop patterns we already shipped and must not repeat:**

| Slop | What happened | Lesson |
|------|----------------|--------|
| Graph UI with no reliable V2 | Staging showed `nodes 0/0` while UI said “live”; V2 hung/unhealthy | Always surface **service down** vs empty; health + autoheal + bridge re-project |
| Team map without proven 2-human digests | Code/API ready; **not** validated as a team product | Multi-person is **not done** until 2 humans get correct rare digests |
| Intent/conflict “v0” thin | Rules exist; little real multi-person conflict evidence on live data | Structure-first OK, but don’t oversell differentiator until data shows it |
| Linear “next” forever | Deferred while UI sprawl continued | One connector fully productized > three half connectors |
| Veto jargon without UX | Founder (and users) don’t know what “veto” means | Prefer **Approve / Edit / Don’t send** in product copy |
| Half-hour Slack nags | `COMPILE`/`NOTIFY` ~1800s + new status windows re-issued same open PR story | **Notify Policy v1** (change-only + daily cap) — ingest continuous, Slack rare |
| Confidence score shock | Honest ~45% of full vision / ~30% stranger-pilot confidence felt like insult | Separate **vision breadth** vs **airtight pilot path**; then **close A1–A7** before pitch |
| Shabby one-liner | “Founder-operated pipeline” is not a product pitch | Position on **job-to-be-done** only after anti-spam + multi-person work |

**Moving forward:** Prefer fewer features, each **fully done**, over milestone theater.

---

## 1. Product thesis (non-negotiable)

| Stance | Detail |
|--------|--------|
| **What we are** | Permissioned **engineering context plane** + **meeting elimination** (status from real work) |
| **Anti-Glean** | No vector search, no full-text corpus, no proprietary source hosting (ADR-006) |
| **Not Buzz / Centaur** | Not multiplayer workspace; steal egress inject only (ADR-011/012) |
| **Success** | Meetings deleted / focus reclaimed — not engagement rankings |
| **Slack** | Continuous **ingest**; **batched / change-only** notify (ADR-014 + Notify Policy v1) |
| **Private DMs** | No silent 1:1 wiretap (ADR-015) |
| **Local models** | Only after multi-person digests + learning-window gold (ADR-016) — **not now** |

**Golden path:**

```
Sources (GitHub…) → V1 ingest (ACL) → V2 graph → V3 status twin
  → rare Slack DM (egress, approve/edit/don't-send) / optional channel
```

**Positioning (target after airtight pilot loop — pick one):**

- **Primary:** “Status that writes itself from your PRs — you approve before anyone sees it.”  
- **Alt:** “Kill the standup. Keep the signal.”  
- **Alt:** “Engineering context that posts status, not another search box.”  

**Do not pitch until airtight:** digital twins of everyone, agents that ship code, on-prem custom GPT of the company, search everything.

---

## 2. Honest confidence scores (2026-07-26 planning session)

| Gauge | Score | Meaning |
|-------|------:|---------|
| Full original vision breadth | **~45%** | Tickets, multi-tenant, models, multi-team ops mostly missing |
| Shipped surface that works (solo founder path) | **~70%** of *what was coded* | GitHub → draft → Slack can work |
| Bet life on **stranger team tomorrow** | **~25–30%** before Notify v1; **improving** after anti-spam ship | Still need multi-person proven + pilot package |
| Target for soft outreach | **&gt;50%** | Close **A1–A7** below + 3-day dry-run with 2 people |
| Target for confident pilots | **&gt;70%** | One external partner finishes 10–14d without babysitting |

---

## 3. Remaining gap lists (from planning session)

### A — Must-have for &gt;50% stranger-pilot confidence

| ID | Gap | Airtight done means |
|----|-----|---------------------|
| **A1** | Notify non-spam | **Live-verified 2026-07-30** (staging: 166 suppressed / 2 sent) |
| **A2** | Multi-person digests proven | 2 mapped humans, both get correct rare digests; Graph shows both — **blocked on 2nd human IDs** |
| **A3** | Durable graph on staging | **Hardened 2026-07-30**: recovery mode + Connections graph_status; re-fill target &lt;2 min |
| **A4** | Status loop as finished product | Approve / Edit / Don’t send clear; silence rules in UI |
| **A5** | Partner install runbook | **Shipped** `Design Partner_ Install Runbook.md` |
| **A6** | Empty / wrong-draft UX | **Improved** evidence on items + empty banner (no DM) |
| **A7** | Pilot packaging | **Aligned** one-pager + playbook to real product language |

### B — Vision backlog (not first for outreach)

Linear/Jira productized · Slack channel metadata · self-serve multi-tenant · learning-window state machine · Model Router / on-prem SLM · rich Slack conflict resolution · browser capture  

**Do not expand B until A is boringly reliable.**

---

## 4. Milestone status

| M | Meaning | Status |
|---|--------|--------|
| M0–M4 | Engines + sew + real Slack + batch notify foundations | **Done** |
| M5 | Staging HTTPS product UI + bridge | **Done enough** (`status.neel.world`) |
| **M6** | Multi-member beta path | **In progress** — team map, intent/conflict v0, Graph UI, notify v1, graph harden; **multi-person digests not proven**; Linear not productized |
| M6.5 | Learning window 10–14d + gold export | After first partner |
| M7 | Model Router + customer-prem SLM | After shadow gold only (ADR-016) |

---

## 5. What works now (objective inventory)

### High trust

- Staging: **https://status.neel.world/app/** (light B&W product UI)  
- V1 GitHub webhooks → continuous ingest  
- Bridge: V1→V2 + twin upsert; **never DMs** (ADR-014)  
- V3: compile, draft, approve/edit/don’t-send, channel publish paths, egress Slack  
- **Notify Policy v1:** change-only fingerprint + max 1 status DM/person/UTC day (force only for demo “Send test” / explicit force)  
- Graph live map UI + snapshot API; autoheal + bridge health-gate when V2 dies  
- Intent rules v0 on PR/issue project; conflicts API  
- Team map API/UI (multi-person **plumbing**, not fully proven digests)  
- Design Partner one-pager + learning-window playbook (docs exist)  

### Medium / fragile

- Embedded V2/V3 on staging = memory wipe on container restart (recover via bridge re-project + autoheal)  
- Multi-person: second human **not** mapped in a completed dry-run  
- OAuth self-serve still stubs; secrets vault path is real  
- Conflict surface sparse without multi-person + ticket data  

### Low / not product

- Linear/Jira productized connector  
- Slack channel short-text ingest  
- Self-serve multi-tenant  
- Local model training  

---

## 6. Status action lingo (product language)

| UI / Slack | Meaning | System |
|------------|---------|--------|
| **Approve** | “This is accurate — share it.” | Publish path |
| **Edit** | “Fix the words.” | Saves edited body for publish |
| **Don't send** (internal: veto) | “Wrong — kill this draft.” | No channel post for that ledger |
| **Silence → auto-share** | Ignored DM past deadline | **Medium** may auto-share; **blocker** never silent-publish |

**Why human gate exists:** Without it we are a nag bot inventing status. With it we are status assistance under human authority.

---

## 7. Notify Policy v1 (anti-spam — shipped)

| Rule | Behavior |
|------|----------|
| Content fingerprint | Hash of items + blockers + rollup (not wall-clock window alone) |
| Unchanged story | No new Slack DM |
| Daily cap | Default **1** status DM per person per UTC day (`MAX_STATUS_DMS_PER_DAY`) |
| New blocker | Can break daily cap if fingerprint changed |
| Empty ledger | No DM |
| Force | Demo simulate / explicit force only |
| Metrics | `twin_dms_sent_total`, `twin_dms_suppressed_total`, `notify_policy: v1_change_only_daily_cap` |

Code: `vertical-3/crates/twin-core/src/notify_policy.rs`, delivery worker, product UI My status.

---

## 8. Staging / ops

| Item | Value |
|------|--------|
| URL | https://status.neel.world/app/ |
| Host | DO VPS `206.189.129.31` — **always on** unless founder says otherwise |
| Compose | `deploy/docker-compose.app.yml` + TLS profile |
| Autoheal | Restarts containers labeled `autoheal=true` (V2) when unhealthy |
| Bridge | Health-gates V2; re-projects when graph empty; poison-skip stuck events |
| Secrets | Only egress vault / host gitignored files (ADR-012) |
| SSH | `~/.ssh/id_ed25519` → `neel@206.189.129.31` (repo `ssh/` key may be encrypted) |

**Ports (local/staging internal):** V1 18080 · V2 18082 · V3 18083 · egress 18090  

---

## 9. Sprint plan to recruitable pilot (ordered)

### Sprint 1 — Trust & anti-spam  
- [x] Notify Policy v1  
- [x] Approve / Edit / Don’t send copy  
- [x] Live verification via `/metrics` (2026-07-30: suppressed ≫ sent); optional founder 48h feel-check  

### Sprint 2 — Multi-person airtight  
- [ ] Map 2nd human (fields: display_name, slack_user_id U…, github_login, github_numeric_id, tenant `ten_github`)  
- [ ] Both digests correct; Graph shows 2 people; no default-Slack spam for unmapped actors  

### Sprint 3 — Pilot package  
- [x] Install runbook for founder-operated partner install  
- [x] Align Design Partner one-pager + playbook to **actual** product behavior  
- [ ] Soft outreach only after Sprint 2 green (A2)  


### Sprint 4 — Only if partners demand  
- [ ] Linear **fully** productized (one surface, airtight) **or** Slack channel metadata — not both half-done  

### Explicitly later  
- Model Router / Ollama / LoRA (ADR-016)  

---

## 10. Security rules (never break)

- Long-lived tokens only in egress vault / host secrets — not twin env  
- No god-mode SQL from V3 into `context_graph`  
- No LOC / productivity rankings  
- No silent private 1:1 Slack wiretap  

---

## 11. Recent commits (reference)

- (2026-07-30) A3 recovery mode + A5 install runbook + A1 live verify notes + status UX polish  
- `fc0b662` — 2026-07-27 session handoff + airtight pilot plan  
- `d1e660b` — Notify Policy v1 + Approve/Don’t send  
- `7217a0e` / `6ce4c91` — Graph harden + autoheal + V2 concurrency  
- `866dbd4` — Live Graph panel  
- `db7b4cd` — M6 team map + intent/conflict v0 + pulse  

---

## 12. Files to attach / read first (new session)

**Always (ground truth):**

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-07-27.md` ← **this file**  
2. `plans/2026-07-27_confidence-airtight-pilot.md`  
3. `starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` (esp. ADR-011–016)  
4. `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`  
5. `starting-out-documents/Interaction Log_ Product Decisions.md`  
6. `starting-out-documents/Design Partner_ One-Pager.md`  
7. `starting-out-documents/Design Partner_ Learning Window Playbook.md`  
7b. `starting-out-documents/Design Partner_ Install Runbook.md`  

**Product / eng:**

8. `vertical-3/crates/twin-core/src/notify_policy.rs`  
9. `vertical-3/crates/twin-delivery/src/worker.rs`  
10. `vertical-3/app-static/index.html` + `app.js`  
11. `scripts/github_live_bridge.py`  
12. `deploy/docker-compose.app.yml` + `deploy/README.md`  
13. `plans/2026-07-24_m6-multi-member-beta.md`  
14. `plans/2026-07-24_onprem-model-and-agents.md` (sequence lock only — do not train yet)  

**Optional architecture:**

15. `vertical-3/Technical Architecture Specification_ Vertical 3.md`  
16. `vertical-2/Technical Architecture Specification_ Vertical 2.md`  

---

## 13. Commands

```bash
# Local
./scripts/dev_up.sh
open http://127.0.0.1:18083/app/

# Verify
cd vertical-1 && cargo run -p telemetry-verify
cd vertical-2 && cargo run -p graph-verify && cargo test -p graph-core
cd vertical-3 && cargo run -p twin-verify && cargo test -p twin-core notify_policy

# Staging (from monorepo; images often built linux/amd64 on Mac → load on droplet)
# See deploy/README.md + deploy/scripts/sync_and_deploy_staging.sh
```

---

## 14. Autonomous until / stop for human

**Autonomous:** Notify polish, multi-person dry-run plumbing, graph durability, pilot docs alignment, Linear **only if** secrets ready and A2 green.  

**Stop for human:** Second human Slack/GitHub IDs; design-partner identity; Linear/Jira OAuth secrets; droplet power-off (assume always on); anything that would half-ship a new surface.

---

## 15. Prompt to paste into the next session

Copy the block in **§15 of this file** (also duplicated at end of `plans/2026-07-27_confidence-airtight-pilot.md`) and attach the files in §12.

```
You are continuing the AI Manager monorepo (private GitHub neeljoshi18/AI-Manager, branch main).

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-07-27.md
2. plans/2026-07-27_confidence-airtight-pilot.md
3. Architecture Decision Log (ADR-011–016)
4. Product Roadmap + Interaction Log
5. Design Partner one-pager + learning-window playbook
6. notify_policy.rs + twin-delivery worker + app-static + github_live_bridge.py + deploy/docker-compose.app.yml

Ground rules:
- NEVER half-ass. Ship airtight end-to-end or don't ship. No feature theater.
- Product: permissioned engineering context + meeting elimination. Anti-Glean. Not Buzz/Centaur.
- Continuous ingest; Slack notify BATCHED + Notify Policy v1 (change-only, daily cap). Bridge never DMs.
- Secrets only via egress vault. No silent private 1:1 wiretap. No local model training yet (ADR-016).
- Prefer Approve / Edit / Don't send language over "veto" in product UI.
- Staging: https://status.neel.world/app/ — droplet always on unless I say otherwise.
- Save plans under plans/YYYY-MM-DD_slug.md. Commit and push when slices complete.

Mission: raise stranger-pilot confidence past 50% by finishing A-list gaps:
A1 notify (verify live) · A2 multi-person digests proven · A3 graph durability · A4 status UX · A5–A7 pilot package.
Do NOT start Linear/training until A2 is proven unless I provide secrets and explicitly ask.

Start by confirming you read the 2026-07-27 handoff + airtight ground rule + Notify Policy v1, then implement the highest-value incomplete A-item without waiting unless blocked on human secrets.
```

---

## 16. Document control

| Field | Value |
|-------|--------|
| Owner | Founder + coding agents |
| Related plan | `plans/2026-07-27_confidence-airtight-pilot.md` |
| Prior handoff | `Session Handoff_ Context Transfer 2026-07-23.md` |
