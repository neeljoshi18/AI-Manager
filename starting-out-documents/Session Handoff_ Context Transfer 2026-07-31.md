# Session Handoff — Context Transfer

**Date:** 2026-07-31  
**Repo:** private monorepo `https://github.com/neeljoshi18/AI-Manager` (branch `main`)  
**Purpose:** Full handoff for a **new chat**. Prefer this file + listed ground-truth docs over compacted history.  
**Supersedes for “what to do next”:** `Session Handoff_ Context Transfer 2026-07-27.md` (keep for older ports/history).  
**Related plans:** `plans/2026-07-27_confidence-airtight-pilot.md`, `plans/2026-07-31_batch-until-deploy.md`, `plans/2026-07-31_pilot-autonomy-backlog.md`

---

## 0. Standing rules (forever — update this file when they change)

| Rule | Detail |
|------|--------|
| **Airtight or don’t ship** | End-to-end, observable, non-spammy, stranger-readable. No feature theater. |
| **Product** | Permissioned engineering context + meeting elimination. Anti-Glean. Not Buzz/Centaur. |
| **Ingest continuous; Slack rare** | Notify Policy v1 (change-only + daily cap). Bridge never DMs. |
| **Secrets** | Egress vault only. No silent private 1:1 wiretap. No local model training yet (ADR-016). |
| **UI language** | Approve / Edit / Don’t send (not “veto” in product UI). |
| **Staging** | https://status.neel.world/app/ — droplet always on unless founder says otherwise. |
| **Plans** | Save under `plans/YYYY-MM-dd_slug.md`. Commit and push when slices complete. |
| **Campus Wi‑Fi SSH** | Outbound **TCP 22 often blocked** (timeout). Hotspot works. Prefer **GitHub Actions deploy** after secrets set. Agent must **not spam SSH**. |
| **Batch deploys** | Build + push to `main` continuously; **one deploy** when founder has hotspot or Actions secrets — don’t redeploy every commit. |
| **Agent autonomy** | Do terminal/git/HTTPS work yourself. Only stop for: secrets founder must paste, legal/partner identity, campus-blocked SSH (ask for hotspot or use Actions). |
| **Handoff hygiene** | **Always update this handoff (and ADR / Interaction Log / plans) when we make key product or ops decisions.** |

---

## 1. Prompt to paste into the next session

```
You are continuing the AI Manager monorepo (private GitHub neeljoshi18/AI-Manager, branch main).

Read first (in order):
1. starting-out-documents/Session Handoff_ Context Transfer 2026-07-31.md  ← THIS FILE (full context)
2. plans/2026-07-31_batch-until-deploy.md
3. plans/2026-07-27_confidence-airtight-pilot.md
4. starting-out-documents/Architecture Decision Log_ Pivotal Choices.md (ADR-011–016)
5. Design Partner One-Pager + Install Runbook + Learning Window Playbook
6. deploy/scripts/setup_ssh_via_https_port.md (campus Wi-Fi deploy path)
7. Code: notify_policy.rs, twin-delivery worker, twin-compiler lib, twin-api main, github_live_bridge.py, app-static, docker-compose.app.yml

Ground rules:
- NEVER half-ass. Ship airtight end-to-end or don't ship.
- Product: permissioned engineering context + meeting elimination. Anti-Glean. Not Buzz/Centaur.
- Continuous ingest; Slack BATCHED + Notify Policy v1. Bridge never DMs.
- Secrets only via egress vault. No silent 1:1 wiretap. No local model training yet (ADR-016).
- Prefer Approve / Edit / Don't send over "veto" in product UI.
- Staging: https://status.neel.world/app/ — droplet always on unless I say otherwise.
- Campus Wi-Fi blocks SSH:22. Do NOT spam SSH. Prefer git push → GitHub Actions deploy once secrets are set. Batch builds on main; one deploy later unless I say "deployed" / "hotspot ready".
- Save plans under plans/YYYY-MM-DD_slug.md. Commit and push when slices complete.
- ALWAYS update this handoff + Interaction Log / relevant ADRs/plans when making key decisions.

Mission: raise stranger-pilot confidence past 50% — finish A-list gaps; digests with real multi-person content; keep product recruitable.
Do NOT start Linear/training until A2 digests are proven unless I provide secrets and explicitly ask.

Start by confirming you read the 2026-07-31 handoff (SSH/campus rules, batch deploy, what shipped, mental notes, execution order), then implement the next incomplete item without waiting unless blocked on human secrets or a deploy I must run.
```

---

## 2. Files to attach / read first (new session)

### Always (ground truth)

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-07-31.md` ← **this file**  
2. `plans/2026-07-31_batch-until-deploy.md`  
3. `plans/2026-07-27_confidence-airtight-pilot.md`  
4. `plans/2026-07-31_pilot-autonomy-backlog.md`  
5. `starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` (esp. ADR-011–016)  
6. `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`  
7. `starting-out-documents/Interaction Log_ Product Decisions.md`  
8. `starting-out-documents/Design Partner_ One-Pager.md`  
9. `starting-out-documents/Design Partner_ Install Runbook.md`  
10. `starting-out-documents/Design Partner_ Learning Window Playbook.md`  

### Deploy / campus Wi‑Fi

11. `deploy/scripts/setup_ssh_via_https_port.md`  
12. `deploy/scripts/deploy_when_ssh.sh`  
13. `deploy/scripts/ssh_staging.sh`  
14. `.github/workflows/deploy-staging.yml`  
15. `deploy/docker-compose.app.yml`  
16. `deploy/README.md`  

### Product / eng (core paths)

17. `vertical-3/crates/twin-core/src/notify_policy.rs`  
18. `vertical-3/crates/twin-delivery/src/worker.rs`  
19. `vertical-3/crates/twin-compiler/src/lib.rs`  
20. `vertical-3/crates/twin-compiler/src/http_v2.rs`  
21. `vertical-3/crates/twin-api/src/main.rs`  
22. `vertical-3/crates/twin-core/src/store.rs` (embedded twin persist)  
23. `vertical-1/crates/telemetry-core/src/store.rs` + `acl.rs` + `wiring.rs` (event + identity persist)  
24. `vertical-2/crates/graph-core/src/store.rs` + `graph-api/src/main.rs` (graph persist + intent seed + person collapse)  
25. `scripts/github_live_bridge.py`  
26. `vertical-3/app-static/index.html` + `app.js`  

### Optional prior context

27. `starting-out-documents/Session Handoff_ Context Transfer 2026-07-27.md`  
28. `plans/2026-07-24_m6-multi-member-beta.md`  
29. `plans/2026-07-24_onprem-model-and-agents.md` (sequence lock only — do not train)  

---

## 3. Mental notes (do not lose)

### Ops / environment

1. **College Wi‑Fi blocks outbound SSH (TCP 22)** → agent SSH times out; founder hotspot works.  
2. **Deploy from campus without hotspot** → GitHub Actions (`Deploy staging` workflow) after one-time secrets: `STAGING_HOST`, `STAGING_USER`, `STAGING_SSH_KEY`.  
3. **Optional interactive SSH on campus** → sshd **port 2222** (not 443 — Caddy owns 443).  
4. **Batch deploys:** many commits on `main`, **one** deploy to test everything.  
5. **Droplet:** `206.189.129.31`, user `neel`, UI `https://status.neel.world/app/`, always on unless founder says off.  
6. **DO firewall:** 80/443 must be open publicly (we fixed timeout that was firewall, not app).  
7. **Secrets never in CI** — `dev_secrets.json` + `deploy/.env.staging` stay only on host.  
8. **Agent:** do git/HTTPS/tests yourself; stop only for founder secrets / hotspot / partner IDs.  

### Product / architecture

9. **`ten_github`** = our pilot tenant id (not a GitHub setting).  
10. **Notify Policy v1** shipped and live-proven (suppress ≫ sent). Metrics: `twin_dms_suppressed_total`, `notify_policy`.  
11. **Embedded mode RAM wipe** was root cause of empty graph after rebuild — fixed with **disk journals** (V1 events, V1 ACL identity, V2 graph, V3 twins).  
12. **Pre-durability history is gone forever** — cannot reconstruct old graph from us; only new webhooks + seed.  
13. **Multiple floating “neeljoshi18”** = new `gu_*` each restart (ACL identity not persisted) + seed twins + graph overlay of every twin. Fixes: ACL persist, prune one-twin-per-Slack, UI/API person collapse.  
14. **Digests were empty** for pure git activity because compiler only ledgered PR/issue, not commits/pushes — fixed on main.  
15. **paneerjeera compile 404** = V2 neighborhood ACL (no grp_eng) — fixed with ensure_users + soft-fail empty neighborhood.  
16. **Twin subject must align with person node that has edges** (or compile across `gu_*` aliases — now on main).  
17. **Intent demo** = SHIP vs FREEZE + BLOCKED for Team blockers (`POST …/seed/intent_demo`).  
18. **Bridge never DMs**; use Team `/members` + prune for twins.  
19. **No Linear / local models** until multi-person digests proven.  
20. **UI:** Approve / Edit / Don’t send; hide demo alice/bob by default.  

### People map (staging pilot)

| Person | Slack | Notes |
|--------|-------|--------|
| neeljoshi18 | `U0APK7W1X99` | GitHub login/id `222674398` |
| paneerjeera / Neel Joshi (2nd) | `U0BLN0N7VB5` | GitHub id `309182469` |

`multi_person_ready` must mean **≥2 distinct Slack user IDs**, not 3 aliases for one person.

### Staging lag mental note

At last check from agent, **live staging was behind `main`** (`ensure_users` 404, no hide-demo in HTML). After founder deploys once, smoke:

```bash
curl -sS https://status.neel.world/healthz
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/graph/ensure_users
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/team/prune
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/seed/intent_demo
curl -sS -X POST https://status.neel.world/v3/tenants/ten_github/team/compile \
  -H 'content-type: application/json' -d '{"force_notify":false,"allow_notify":true}'
```

---

## 4. What we did this session (summary)

### A-list / pilot confidence

- Confirmed Notify Policy v1 live (high suppress count).  
- Mapped second person (paneerjeera); multi-person team path.  
- Shipped **embedded durability**: V1 events, V1 ACL identity map, V2 graph snapshot, V3 twins (volumes in compose).  
- **Intent/conflict seed** for Team blockers (SHIP vs FREEZE, BLOCKED).  
- **Prune** duplicate Slack twins; graph collapse / hide demo.  
- Expanded digests to **commits/pushes/repos**; soft-fail neighborhood 404.  
- Multi-identity compile (merge graph views across `gu_*` aliases).  
- Partner install runbook, one-pager align, learning-window language.  
- Staging HTTPS outage diagnosed as **DO firewall 80/443** (not app).  
- Deploy script: don’t die on sudo swap password.  

### Ops paths

- Documented campus SSH block.  
- **GitHub Actions** workflow `Deploy staging` for push-from-campus.  
- Helpers: `deploy_when_ssh.sh`, `ssh_staging.sh`, setup doc.  
- Standing mode: batch on main, one deploy later.  

---

## 5. What went wrong → how we fixed

| Problem | Cause | Fix |
|---------|--------|-----|
| SSH timeout from agent | Campus Wi‑Fi blocks :22 | Hotspot for emergency; **Actions deploy** for normal; no SSH spam |
| Public HTTPS timeout | DO firewall missing 80/443 | Open HTTP/HTTPS in DO firewall |
| Deploy died after rsync | Remote `sudo` for swap needs password | Swap best-effort; compose always runs |
| Graph empty after redeploy | Embedded V1/V2 in-memory wipe; V1 empty → bridge can’t re-project | Disk journals + volumes; re-project after new events |
| “All past activity gone” | Never had durable SoT for embedded | Persist going forward; old history not recoverable |
| Multiple floating Neels | New `gu_*` per restart + seed twins + overlay all twins | ACL identity persist; prune; UI/API collapse |
| Team multi_person_ready true with 1 human | Counted map rows / same Slack thrice | Unique Slack among enabled twins |
| Digests empty with git activity | Compiler only PR/issue | Ledger commits/pushes/repos (cap commits) |
| paneerjeera compile hard-fail | V2 neighborhood 404 (ACL/membership) | ensure_users; soft empty view; scheduler seed membership |
| existing_draft forever blocks upgrade | Empty placeholder draft never re-notified | Fall through when empty→has items |
| Staging behind main | Deploys incomplete / old image | One full `deploy_when_ssh` or Actions after secrets |
| Agent asked founder for every deploy | SSH friction | Batch on main; single deploy; Actions later |

---

## 6. Order of execution (next session — follow this)

### Phase A — Orient (5 min)

1. Read this handoff §0–§5.  
2. `git log -5 --oneline` on `main`.  
3. HTTPS smoke only (no SSH): healthz, team, pulse, metrics, `ensure_users` status code.  

### Phase B — If founder says “deployed” or Actions green

1. ensure_users → prune → seed intent → team compile.  
2. Verify digests non-empty when graph has commits for mapped gu_*.  
3. Graph: one primary neeljoshi18 + paneerjeera; demo alice/bob hidden by default.  
4. Update this handoff status table.  

### Phase C — Product work while batching (no deploy)

1. ~~Dual-person digests: activity quality (evidence, windows)~~ **done 2026-07-31** — 24h lookback, `valid_from`, open-PR keep, team compile proof fields, unit tests.  
2. Bridge: keep Team API upsert + prune (already).  
3. ~~Hide demo seed from graph server-side~~ **done 2026-08-01** — V2 `include_demo` default false; twin-api belt filter; pulse demotes demo conflicts.  
4. ~~Partner package final pass~~ **done 2026-08-01** — runbook/one-pager/playbook: 24h, ensure_users, demo hide.  
5. ~~Soft-outreach checklist~~ **done** — `plans/2026-08-01_soft-outreach-checklist.md`.  
6. **Still no-deploy options:** UI polish, local tests, bridge map edge cases, Actions secrets docs only. **Live A2 proof still needs deploy.**

### Phase D — When founder has reliable hotspot (once)

1. Add GitHub Actions secrets (`STAGING_*`).  
2. Run workflow once; confirm campus `git push` deploys.  
3. Optionally configure sshd **2222** for interactive shell.  
4. Mark ops path “campus-ready” in this handoff.  

### Phase E — Explicitly later (do not start unprompted)

- Linear productized connector  
- Full Cockroach V1/V2/V3 (beyond embedded files)  
- Local model training / Model Router (ADR-016)  
- Self-serve multi-tenant  

---

## 7. A-list status (pilot confidence)

| ID | Item | Status |
|----|------|--------|
| A1 | Notify non-spam | **Done** (live metrics + policy v1) |
| A2 | Multi-person digests proven | **Lookback + multi-identity + dual-person tests done**; live proof needs post-deploy compile — person1 non-empty if graph activity in 24h; person2 needs real GH edges |
| A3 | Graph durability | **Done in code** (persist volumes); verify after next full deploy |
| A4 | Status UX Approve/Edit/Don’t send | **Done** + empty/evidence polish on main |
| A5 | Partner install runbook | **Done** |
| A6 | Empty/wrong draft UX | **Improved** on main |
| A7 | Pilot packaging | **Done** (package + soft-outreach checklist 2026-08-01) |

**Stranger-pilot confidence:** ~50–58% on **code path** (A2 lookback + demo hide + package); still capped until staging runs latest `main` and dual digests prove live. Target **>50% soft outreach** only after one deploy + A2 live green.

---

## 8. Recent commits (this arc — reference)

```
(this session) A2: 24h activity lookback + dual-person digest proof surface
6be4ae4 / 89b4500 Session handoff 2026-07-31
8a7f2c6 Batch: multi-identity digest compile and scheduler ACL seed
7ceff1c / 5dc1d08 Campus-WiFi GitHub Actions deploy
3888b8e Partner package + richer deploy_when_ssh
9f3bfe9 / 11d1f58 Empty-to-items draft re-notify
94f8f15 Digests commits/pushes + soft-fail neighborhood
… Notify Policy v1, graph harden, multi-person map earlier
```

Confirm tip with `git log -15 --oneline`.

### Mental notes added this session

21. **Empty digests (live):** staging behind main **and** 1h window; activity ~20h old → default 24h rolling lookback + `valid_from`.  
22. **paneerjeera has zero graph edges** — empty digest correct until that GH user pushes/PRs.  
23. **ledger_id** stays wall-aligned; human lookback is rolling `activity_start..activity_end`.  
24. **Demo seed** (alice/bob / demo-repo) hidden server-side by default; pulse keeps demo_* separate so Today is not theater.  
25. **No-deploy efficiency:** keep batching on main; optional founder power-off droplet to save cost until hotspot day (agent will not power-off unprompted).

---

## 9. Commands

```bash
# Local
./scripts/dev_up.sh
# open http://127.0.0.1:18083/app/

# Tests
cd vertical-3 && cargo test -p twin-core && cargo test -p twin-compiler --lib
cd vertical-1 && cargo test -p telemetry-core --lib

# Staging HTTPS (no SSH)
curl -sS https://status.neel.world/healthz
curl -sS https://status.neel.world/v3/demo/status
curl -sS https://status.neel.world/v3/tenants/ten_github/team
curl -sS https://status.neel.world/metrics

# Deploy — only when founder allows (hotspot or Actions secrets)
./deploy/scripts/deploy_when_ssh.sh
# or: gh workflow run deploy-staging.yml
```

---

## 10. Document control

| Field | Value |
|-------|--------|
| Owner | Founder + coding agents |
| Update policy | **On every key decision:** append Interaction Log; update this handoff §0/§3/§6/§7; touch ADRs/plans if architecture/product changes |
| Prior handoff | `Session Handoff_ Context Transfer 2026-07-27.md` |
| Deploy campus path | `deploy/scripts/setup_ssh_via_https_port.md` |
