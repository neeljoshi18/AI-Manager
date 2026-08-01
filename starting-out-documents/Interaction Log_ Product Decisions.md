# Interaction Log — Product & Architecture Decisions

**Purpose:** Durable log of founder ↔ build sessions so future chats don’t rely on compressed context.  
**Repo path:** `starting-out-documents/Interaction Log_ Product Decisions.md`

---

## 2026-07-22 — Session: V3 implement → Sew & Show → GitHub live → batch notify → roadmap

### Context

- Monorepo AI Manager; private GitHub `neeljoshi18/AI-Manager`.  
- V1/V2/security already largely built; V3 was spec-only.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Implement V3 per TAS (twins, ledger, veto, egress Slack) | Product layer where pitch becomes real |
| 2 | **Do not** build Buzz/Centaur clones | ADR-011 |
| 3 | Demo console on `:18083/demo/` | Founder visibility; lead-friendly |
| 4 | Real Slack via egress vault only | ADR-012; no bot token in twin env |
| 5 | Full stack sew V1+V2+V3 before “agent OS” | Empty graph → empty product |
| 6 | Live GitHub via ngrok + webhook secret | Prove real PR path |
| 7 | **Ingest continuous, notify batched** | User got ~15 DMs per PR activity; unacceptable |
| 8 | Defaults: 1h status window, 30m notify interval | Configurable env knobs |
| 9 | Bridge projects graph only; twin-api owns DMs | Separation of ingest vs delivery |
| 10 | Private 1:1 DM wiretap is **out**; bot-mediated / opt-in only | Slack platform + ethics |
| 11 | Next: productize onboarding, ticketing, Slack inbound, then intent/conflict agents | Full app goal without scope collapse |
| 12 | Plan captured in `Product Roadmap_ Intent Capture to Digital Twins.md` | Single backlog spine |

### Artifacts produced

- `vertical-3/` full crates + migrations + demo-static  
- `scripts/platform_sew.sh`, `scripts/github_live_bridge.py`  
- `Human Demo Script.md`, `GitHub Webhook Setup_ Local.md`, `Plan_ Sew and Show M4.md`  
- Git push: commit `1cb28c1` (and subsequent if any)  

### Open questions (for later sessions)

1. GitHub App vs long-lived ngrok for partners?  
2. First ticketing connector: Jira vs Linear?  
3. Cloud host for M5 (Fly / Render / bare VM)?  
4. When to open ADR for **V4 Intent & Conflicts**?  

---

## 2026-07-23 — Session: Demo → product plan + product UI shell

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Focus next on **demo → product** (UX + deploy), not agent sprawl | Trust bottleneck |
| 2 | Intent classification = extract/type/attach/conflict with **rules v0 first** | Evidence, not LLM invent |
| 3 | Agentic layer = **monitors** on graph, human veto | ADR-011 |
| 4 | Create monorepo **`plans/`** for dated plan snapshots | Durable planning history |
| 5 | Product UI at `/app/`; lab remains `/demo/` | Buyer vs engineer surfaces |
| 6 | `dev_up.sh` / wake runbook for laptop sleep | Ops friction |

### Artifacts

- `plans/` + `2026-07-23_demo-to-product-m5.md`  
- `scripts/dev_up.sh`, `dev_down.sh`  
- `vertical-3/app-static/` product shell  
- Wake runbook under `plans/`  

---

## 2026-07-23 — Session: M5 multi-service compose + last-event Connections

### Context

- Continuing from Session Handoff 2026-07-23; M5 “demo → product → staging”.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Ship **embedded** multi-service compose first | Staging/demo without CRDB ops on day one |
| 2 | Caddy profile `tls` for HTTPS path; need human DOMAIN/DNS | Scaffold now; secrets/host later |
| 3 | Product health = **last accepted ingest age**, not only process up | Matches UX pillar / Vercel-like connectors |
| 4 | OAuth/App **manifests only** until human credentials | Stop for secrets (handoff rule) |
| 5 | `V1_BASE_URL` on twin-api | Docker network probes (no hardcoded 127.0.0.1) |

### Artifacts

- `deploy/docker-compose.app.yml`, Dockerfiles V1/V2/egress/twin, `Caddyfile`  
- `deploy/oauth/*`  
- V1 `last_accepted_unix`; Connections UI last-event age  
- Plan: `plans/2026-07-23_m5-multiservice-compose.md`  

### Open questions

1. Host preference still open (VPS vs Fly)?  
2. When human provides Slack/GitHub secrets, wire OAuth callbacks next.  

---

## 2026-07-24 — Session: Learning window + on-prem model + agent sequence

### Context

- Staging live at `status.neel.world`; continuous GitHub ingest + bridge + batched Slack.  
- Founder vision: 10–14d learning/shadow period then prefer **local SLM on customer server** for inference cost + privacy; agents call that model instead of forever-paid cloud APIs.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **Ingest always continuous**; only inference/policy changes after learning window | Company brain needs full exhaust (ADR-014) |
| 2 | **ADR-016 Hybrid Model Router** (rules → optional cloud → customer-prem local) | Cost + privacy without Glean corpus or day-one GPU mandate |
| 3 | Train/adapt on **structured gold** (ledgers, edits, intent labels)—not raw source dumps | ADR-006; structure-first; no inventing work items |
| 4 | **M6 multi-member + thin agents before local train** | Beta value + training data; local model is M7 SKU |
| 5 | Product name **“Learning window”** (10–14d) for shadow | Pitch clarity; not “we stopped watching” |
| 6 | Droplet **always on** for staging | Founder ops preference |

### Artifacts

- `plans/2026-07-24_onprem-model-and-agents.md`  
- ADR-016 in Architecture Decision Log  
- Product Roadmap M6 / M6.5 / M7 sequence updates  
- Session Handoff next-task list  

### Open questions

1. First design partner team identity?  
2. Ollama vs vLLM for first on-prem recipe (default Ollama)?  
3. Linear vs Jira as first M6 ticket connector?  

---

## 2026-07-24 — Session: M6 multi-member beta implementation

### Context

- Execute M6 path (not M7 training): multi-person digests, intent/conflict v0, thin monitors, partner playbook.  
- Sequence lock ADR-016: agents + multi-member before local SLM.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Intent nodes attached at **V2 project** time (rules only) | Continuous structure-first; no LLM invent |
| 2 | Owner-scoped intent ids (`intent:{owner}:{work}`) | Dual-owner conflicts surface |
| 3 | Team map admin in V3 + bridge merges `bridge_slack_map` | Multi-person without redeploy env only |
| 4 | Thin monitors cache **pulse** (no Slack from monitor) | ADR-014 batch notify; UI surfaces conflicts |
| 5 | Linear deferred as next connector slice | Default if undecided; not blocking team map / conflicts |
| 6 | Design-partner one-pager + 10–14d playbook written | Beta outreach gate |

### Artifacts

- `plans/2026-07-24_m6-multi-member-beta.md`  
- `vertical-2/.../intent.rs`, conflicts/intents APIs  
- `/app/` Team view + Today conflicts  
- `Design Partner_ One-Pager.md`, `Design Partner_ Learning Window Playbook.md`  

### Open questions

1. Second human Slack IDs for staging map?  
2. Design partner team identity?  
3. Linear OAuth secrets when ready?

---

## 2026-07-26/27 — Session: Reality check, anti-slop, Notify Policy v1

### Context

- Founder unhappy with half-done surfaces, spammy Slack, and shabby positioning from “honest” low confidence scores.  
- Planning session: score vision vs reality; position only what exists; path to &gt;50% stranger-pilot confidence.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **Airtight or don’t ship** is standing ground rule | Half features destroy trust and pitch |
| 2 | Separate vision % from pilot confidence | 45% vision ≠ worthless core path |
| 3 | **A1–A7** before Linear/models/outreach at scale | Anti-spam + multi-person + package first |
| 4 | Product language: **Approve / Edit / Don’t send** | “Veto” is internal jargon |
| 5 | **Notify Policy v1**: change-only + 1 DM/day | Developer-first; stop 30m same-PR spam |
| 6 | Positioning after airtight: status from PRs you approve | Not “founder-operated pipeline” / not agent OS |
| 7 | Local model still post multi-person + shadow gold | ADR-016 unchanged |

### Artifacts

- `starting-out-documents/Session Handoff_ Context Transfer 2026-07-27.md`  
- `plans/2026-07-27_confidence-airtight-pilot.md`  
- Commit `d1e660b` Notify Policy v1  

### Open questions

1. Second human fields for A2 dry-run?  
2. When to soft-outreach (only after A1 verify + A2)?  

---

## 2026-07-30 — Session: A1 live verify + A3 recovery + pilot package

### Context

- New chat from 2026-07-27 handoff; mission raise stranger-pilot confidence past 50% via A-list.
- Staging reachable over HTTPS; SSH from agent host timed out.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Treat A1 as live-verified from `/metrics` (suppress ≫ sent) | Observable anti-spam; optional founder 48h feel-check only |
| 2 | Do **not** start Linear/training; A2 blocked on 2nd human IDs | Handoff stop rule |
| 3 | Ship A3 recovery mode (burst re-project) + Connections graph_status | Close mystery 0/0; target refill &lt;2 min |
| 4 | Ship A5 install runbook + align A7 docs | Soft outreach needs stranger-readable package |
| 5 | Polish A6 evidence + empty draft banner | Empty = no DM must be visible in product |

### Artifacts

- `scripts/github_live_bridge.py` recovery mode  
- `vertical-3` demo_status graph fields + app-static polish  
- `starting-out-documents/Design Partner_ Install Runbook.md`  
- Updated one-pager, playbook, plan, handoff  

### Open questions

1. Second human map fields for A2 proof?  
2. Deploy to droplet when SSH available (code pushed; staging may lag until compose rebuild)  

---

## 2026-07-30b — Session: A2 plumbing airtight + embedded twin persist

### Context

- Second human fields provided (paneerjeera / U0BLN0N7VB5); mapped live on staging.
- Founder: keep shipping until manual block.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Persist embedded twin state to disk | Staging team maps wiped on twin-api restart — A2/A3 reliability |
| 2 | Seed multi-person from SLACK_USER_MAP on boot | Cold start still multi_person_ready |
| 3 | Team digests board + compile-all endpoint | Prove multi-person without waiting 30m windows |
| 4 | Onboarding steps include map ≥2 + digests | Product truth for pilot path |

### Artifacts

- `TWIN_EMBEDDED_STATE_PATH` + volume `twin_state`  
- `POST /v3/tenants/{t}/team/compile`  
- Team/Today digests UI  

### Open questions

1. Deploy rebuild on droplet (SSH was blocked earlier)  
2. Real GH activity from both logins for non-force digests  

---

## 2026-07-31 — Session: durability, multi-person, campus deploy path, batch mode

### Context

Long session from confidence pilot → empty graph after redeploy → firewall → multi-person map → intent seed → campus SSH blocks → batch builds.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Embedded disk journals (V1 events+ACL, V2 graph, V3 twins) before full CRDB | 4GB droplet; stop mystery empty graph |
| 2 | Pre-durability history is not recoverable | Honest; re-fill via webhooks |
| 3 | Campus Wi‑Fi blocks SSH:22; prefer GitHub Actions deploy | Founder cannot always use hotspot |
| 4 | Batch commits on main; one deploy later | Avoid SSH thrash |
| 5 | Digests include commits/pushes; compile across gu_* aliases | Real git activity was invisible |
| 6 | Handoff hygiene: always update 2026-07-31 handoff + Interaction Log on key decisions | Context window loss |

### Artifacts

- `starting-out-documents/Session Handoff_ Context Transfer 2026-07-31.md`  
- `.github/workflows/deploy-staging.yml`  
- `deploy/scripts/setup_ssh_via_https_port.md`, `deploy_when_ssh.sh`  
- Intent seed, prune, ensure_users, graph collapse  

### Open questions

1. Actions secrets set yet?  
2. Latest main on staging?  

---

## 2026-07-31 — Session: A2 digest lookback (batch, no deploy)

### Context

- Staging lag: `ensure_users` 404; empty digests; graph had commits/pushes ~20h old; `STATUS_WINDOW_SECS=3600`; paneerjeera zero edges.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Default **STATUS_WINDOW_SECS=86400** (24h rolling lookback) | Pilot standup replacement; 1h windows empty on sparse activity |
| 2 | Separate **aligned ledger_id bucket** vs **rolling activity filter** | Stable drafts + honest lookback |
| 3 | Propagate V2 `valid_from` into compiler; open PR/issue always keep | Correct window semantics without dropping WIP |
| 4 | Team compile returns `with_items`, kinds, summaries, `empty_reason` | A2 multi-person proof surface |
| 5 | Do not mark A2 live-green until post-deploy dual digests | Airtight bar; paneerjeera needs real GH edges |
| 6 | No Linear/training until A2 proven | Standing rule |

### Artifacts

- `plans/2026-07-31_a2-digest-lookback.md`  
- twin-compiler / twin-core / twin-api / app-static / compose defaults  

### Open questions

1. Deploy once (hotspot or Actions)?  
2. When will paneerjeera push as second identity?  

---

## 2026-08-01 — Session: no-deploy batch (demo hide + package)

### Context

- Founder delayed on reliable hotspot; VPS cost pressure; keep building on `main` without deploy.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Server-side hide intent_demo seed on Graph snapshot (default) | Client-only hide was fragile; pilot Graph must look real |
| 2 | Pulse primary conflicts/intents exclude demo; keep `demo_*` fields | Today blockers not theater; Load intent demo still available |
| 3 | Pulse multi_person uses unique Slack (match Team API) | Avoid false multi-person readiness |
| 4 | Partner package + soft-outreach checklist without waiting for deploy | A7 + recruitable packaging while A2 live waits |
| 5 | Agent does not power-off droplet unprompted | Cost note only; founder chooses downtime |

### Artifacts

- V2 snapshot `include_demo` + tests  
- twin-api graph filter + pulse split  
- `plans/2026-08-01_soft-outreach-checklist.md`  
- Install runbook / one-pager / playbook updates  

### Open questions

1. Actions secrets set?  
2. Power-off droplet until hotspot? (founder only)

---

## Template for future entries

```markdown
## YYYY-MM-DD — Session title

### Context
### Decisions (table)
### Artifacts
### Open questions
```

---

*Append only; never rewrite history of past decisions—supersede with a new entry.*
