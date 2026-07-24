# Product Roadmap — Intent Capture → Digital Twins → Conflict Resolution

**Date:** 2026-07-22 (updated 2026-07-24)  
**Status:** Living plan (post-M5 staging foundation)  
**Related:** Session Handoff; ADR-006/011/012/013/014/015/**016**; V1–V3 TAS; `plans/2026-07-24_onprem-model-and-agents.md`  
**Purpose:** Single place for **what we build next**, expanded scope, privacy rules, and how it stays on-thesis.

---

## 1. North star (unchanged)

| We are | We are not |
|--------|------------|
| Permissioned **engineering context plane** | Glean-class full-text/vector search |
| **Meeting elimination** via evidence-backed status | Buzz multiplayer workspace / Git forge |
| **Intent capture** for conflict resolution & twins | Centaur multiplayer agent OS (steal egress only) |
| Success: meetings deleted / focus time reclaimed | Engagement rankings / LOC surveillance product |
| **Continuous ingest**; optional **customer-prem inference** after learning window | Forever paying cloud token tax for every digest rewrite |

**Spine (already shipping in code):**

```
Sources → V1 ingest (ACL) → V2 org graph → V3 status twins → Slack (egress, veto-first, batched notify)
```

**Target spine (agents + Model Router — ADR-016):**

```
Sources → V1 → V2 → agents/workers → Model Router (rules | cloud via egress | local SLM)
                                              ↓
                                    V3 ledger / Slack (veto, batched)
```

Everything below **extends this spine**. It does not replace it.

---

## 2. Where we are now (truth)

| Layer | Status |
|-------|--------|
| V1 GitHub webhooks | **Live** on staging (`status.neel.world`) + GitHub App |
| V2 graph projection | Live (bridge + project API) |
| V3 ledgers + veto + product UI `/app/` | Live |
| Slack **outbound** DM/channel | Live via egress |
| Batched notify (not 1:1 webhook→DM) | Live (`NOTIFY_INTERVAL_SECS`, etc.) |
| Always-on V1→V2 bridge + twin upsert | Live on staging |
| Slack **inbound** (read channels/DMs) | **Not built** (bot scopes may allow later) |
| Jira / Linear / docs / browser | Normalizers partial (Jira etc. in V1); not productized |
| Multi-agent / conflict resolver | Spec only → **M6** |
| Customer-prem Model Router / local SLM | **Planned ADR-016** (after M6 shadow gold) |
| Self-serve onboarding / deploy | M5–M7 |

---

## 3. Developer day touchpoints (intent surface map)

What a developer actually touches — and how we capture it **without** becoming spyware.

| Touchpoint | Capture method | Priority | Notes |
|------------|----------------|----------|--------|
| **GitHub / GitLab** (PR, push, review) | Webhooks (V1) | **P0 done** | High volume OK; notify batched |
| **Jira / Linear / Asana** | Webhooks | **P0** | Tickets = intent + blockers |
| **Slack channels** (team) | Events API / RTM | **P1** | Metadata + short preview only (≤280) |
| **Slack private channels** | Same + explicit install | **P1** | ACL = channel membership |
| **Slack DMs (human↔human)** | See §4 — **hard** | **P2** | Consent-first; not default |
| **Slack DMs (human↔bot)** | Events to twin-api | **P1** | Edit/veto already; expand Q&A later |
| **Docs** (Notion/Confluence/wiki) | **Not full crawl** | **P2** | Metadata links / page titles only; anti-Glean |
| **Browser learning** (StackOverflow, RFCs) | Optional **browser extension** or OS agent | **P2** | Opt-in; host+title only, no page body dump |
| **IDE / editor** | Language server / extension | **P3** | File path metadata only, no source hosting |
| **Calendar / meetings** | Google/Outlook API | **P2** | Meeting titles = status context |
| **CI / deploy** | GitHub Actions webhooks | **P1** | Ship signal for High confidence |
| **Email** | Usually avoid | **P3** | High privacy cost |

**Principle:** Prefer **collaborative exhaust + explicit opt-in sensors** over continuous screen recording.

---

## 4. Private DMs — honest answer

### Can our bot “just read” your DM with a colleague?

**No — not with a normal Slack bot.**

| Mechanism | What it can see |
|-----------|-----------------|
| Bot token + Events API | Messages **in channels/DMs where the bot is a member** |
| Human↔human 1:1 DM | **Not visible** unless one party **adds the bot** or uses Enterprise **Compliance / Discovery** APIs (org-admin, legal) |
| App Home / slash commands | User-initiated capture |

### Product patterns we **will** support (privacy-safe)

1. **Bot-mediated help**  
   User DMs the **AI Manager bot**: “I’m blocked on auth with @alice.”  
   → Intent event with consent, links people/tickets.

2. **Opt-in “log this thread”**  
   Slash command `/ai-manager capture` in a channel or multi-party DM where bot is invited.

3. **Shared channel / huddle notes**  
   Team chooses public-to-team surfaces for status truth.

4. **Enterprise compliance mode** (later, optional SKU)  
   Only with customer legal sign-off; still ACL-scoped; never personal surveillance product.

5. **Never**  
   Silent reading of private 1:1s without both parties’ org policy + technical consent path.

**Implication for digital twin:** Private interpersonal help is captured when **users choose a visible surface** (bot DM, shared channel, ticket), not by wiretapping.

---

## 5. Intent capture → conflict resolver (product architecture)

```
                    ┌─────────────────────┐
   Sources          │  INTENT LAYER (new)  │
   (webhooks +      │  claims, goals,      │
    opt-in sensors) │  blockers, decisions │
                    └──────────┬──────────┘
                               │
         ┌─────────────────────▼─────────────────────┐
         │  V2 Context Graph (entities + temporal)    │
         │  + Intent nodes/edges (PROPOSES, BLOCKS,   │
         │    DECIDES, SUPERSEDES)                    │
         └─────────────────────┬─────────────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
        Status twins     Conflict resolver   Agent monitors
        (V3 ledgers)     (detect clash of    (sub-agents on
                          goals/owners)       graph diffs)
```

**Conflict resolver (definition):**  
Given two intent claims (e.g. “ship auth rewrite this week” vs “freeze platform”), emit a **structured conflict** with evidence node IDs, owners, and a **human-gated** resolution path (Slack) — not autonomous code changes.

**Employee / person profile (digital twin substrate):**

| Field class | Source |
|-------------|--------|
| Work items authored | Graph AUTHORED |
| Blockers | BLOCKS edges |
| Status narrative | V3 ledger snapshots |
| Stated intent | Bot DMs + tickets + opt-in capture |
| Skills / focus | Aggregated labels over time (not LOC rank) |

---

## 6. Agentic infrastructure (in-thesis)

Aligned with **ADR-011**: agents **monitor and draft**, humans **veto**.

| Agent role | Input | Output | Gate |
|------------|-------|--------|------|
| **Ingest watcher** | Bus lag, poison events | Alerts | Ops |
| **Graph delta monitor** | New edges/nodes | Intent candidates | Deterministic rules first |
| **Status compiler** | Graph window | Ledger + optional DM | Batch schedule + veto |
| **Conflict detector** | Competing intents | Conflict card | Human resolve in Slack |
| **Negotiator (later)** | Twin A/B team-visible blockers | Proposed plan | Human before external post |

**Stack bias:** Tokio workers + graph APIs + egress tools; **not** day-one K8s sandbox swarm.

---

## 7. Product surface: E2E + onboarding

### 7.1 Client onboarding (M5–M7) — **Learning window**

1. **Create workspace** (tenant)  
2. **Connect Slack** (OAuth install — bot + optional events)  
3. **Connect GitHub** (GitHub App preferred over manual webhooks)  
4. **Connect Jira/Linear** (OAuth + webhooks)  
5. **Map users** (provider id → global_user_id → Slack) — **multi-person required for beta**  
6. **Learning window / shadow** **10–14 days**: ingest **continuous**; digests private; high auto-publish **off**; edits/vetoes = training gold  
7. **First standup kill** (enable Medium silence / High auto carefully)  
8. **Optional: local Model Router** (customer-prem SLM after gold pairs) — ADR-016  
9. **Admin console** (health, connectors, last ledger, ACL audit, veto metrics)  

**UI name:** “Learning window” — not “we stopped watching.” Ingest never stops.

### 7.2 Ease-of-use gaps to close

| Gap | Fix |
|-----|-----|
| ngrok + manual webhook | GitHub App + hosted tunnel / cloud deploy |
| CLI-only ops | Onboarding wizard + demo console evolution |
| No multi-tenant SaaS | M7 packaging |
| Spam risk | Batched notify (done); connector-level rate UI |

---

## 8. Phased build list (ordered)

### Phase M4 close (now → 1 week)

- [x] V3 + demo + real Slack outbound  
- [x] GitHub live + batched notify  
- [x] GitHub App **manifest scaffold** (install still needs human secrets)  
- [x] One-command `docker compose` app profile (`deploy/docker-compose.app.yml`)  
- [x] Admin: last-event age on Connections (process + accepted count)  

### Phase M5 — Staging product

- [x] Multi-service container path + Caddy HTTPS scaffold  
- [x] Single-tenant public host + DNS/TLS (`status.neel.world`)  
- [x] Slack bot via egress vault + OAuth env scaffolding  
- [x] GitHub App install + webhook HMAC on staging  
- [x] Always-on bridge (V1→V2 + twin upsert)  
- [x] Health dashboard last-event age + deploy runbooks  
- [ ] Secrets vault path beyond file (later)  
- [ ] OAuth callback → vault write (when polishing self-serve)  

### Phase M6 — Design partner / multi-member beta path (**next engineering**)

- [x] Multi-person Slack map UX (2+ humans, not founder-only) — `/app/` Team + bridge merge  
- [ ] Jira **or** Linear webhook path productized  
- [ ] Slack **channel** message ingest (metadata + short text)  
- [ ] Bot-DM intent capture (“I’m working on X”)  
- [x] Intent classification **v0** (rules → graph)  
- [x] Conflict detector **v0** (rule-based on BLOCKS + dual owners)  
- [x] Thin monitor workers: ingest health, graph delta, conflict surface  
- [x] Metrics stubs: DMs sent, veto rate, empty windows (standups = human report)  
- [x] Design-partner one-pager + shadow playbook  

**Beta outreach gate:** multi-person digests + veto loop + at least one team-visible conflict/blocker surface. **Local model is not a gate.**

### Phase M6.5 — Learning window program (10–14 days)

- [ ] Product copy + admin: Learning window state machine  
- [ ] Export **training pairs** (structured ledger → approved draft text; intent labels)  
- [ ] Scorecard: DMs sent, veto %, empty windows, standups canceled  
- [ ] Partner finishes shadow with go/no-go for auto-publish  

### Phase M7 — On-prem inference SKU + self-serve depth (ADR-016)

- [ ] **Model Router** abstraction (`rules` | `cloud` | `local`) for rewrite / intent / conflict prose  
- [ ] Customer-prem serve recipe (Ollama or vLLM) + open base model  
- [ ] Distill/LoRA job on **approved** pairs only (no raw source corpus)  
- [ ] Multi-tenant control plane  
- [ ] Onboarding wizard  
- [ ] Person profile API (digital twin read model)  
- [ ] Sub-agent monitors as first-class workers  
- [ ] Billing / seats (if SaaS)  

### Phase M8+ — Advanced (parked)

- [ ] Enterprise Slack compliance ingest (legal-gated)  
- [ ] Twin-to-twin negotiation  
- [ ] Docs metadata connectors (Notion/Confluence) without full-text index  
- [ ] IDE extension  
- [ ] Optional browser extension (opt-in host/title)  

### Sequence lock (founder rule 2026-07-24)

```
M5 staging  →  M6 multi-member + thin agents  →  design-partner shadow (10–14d)
     → collect gold  →  M7 Model Router + local SLM  →  richer agents
```

Do **not** implement full local training before multi-person product + one shadow program.

---

## 9. Scope updates (features we **will** and **won’t**)

### In scope (new)

| Feature | Vertical owner |
|---------|----------------|
| Batched status notify | V3 |
| Ticketing connectors (Jira/Linear) | V1 + V2 mappers |
| Slack inbound team surfaces | V1 normalize + V2 |
| Bot-mediated intent capture | V3 + V1 events |
| Conflict cards | V2/V3 or new `vertical-4-intent` |
| Onboarding + deploy | Platform / M5 |
| Sub-agent monitors | Platform workers |
| Learning window + training-pair export | V3 + Platform |
| Model Router + customer-prem SLM | Platform / M7 (ADR-016) |

### Explicitly out / constrained

| Feature | Rule |
|---------|------|
| Silent private DM wiretap | **No** |
| Full doc corpus + vectors | **No** (ADR-006) |
| Train on raw git / Drive dumps | **No** — structured ledgers + approved text only |
| Individual productivity rankings | **No** |
| Full Centaur sandboxes as product | **No** (ADR-011) |
| Hosting proprietary source | **No** |
| Local model **before** multi-member beta path | **No** — sequence lock §8 |

---

## 10. Vertical / workstream naming (proposed)

| Name | Role |
|------|------|
| **V1** | Telemetry / connectors / ACL membership |
| **V2** | Org + intent graph |
| **V3** | Status twins / delivery / bot UX |
| **V4 Intent & Conflicts** (new when started) | Intent nodes, conflict resolver, agent monitors — **M6** (M5 staging green enough) |
| **vertical-security** | Egress / secrets |
| **Platform** | Deploy, onboarding, multi-tenant, **Model Router / on-prem serve (M7)** |

Do **not** start V4 coding until M5 checklist is mostly green (**done enough**).  
Do **not** start local model training code until multi-member beta path + shadow gold (ADR-016).

---

## 11. Config knobs already live (status batching)

| Env | Default | Role |
|-----|---------|------|
| `STATUS_WINDOW_SECS` | 3600 | Ledger window |
| `NOTIFY_INTERVAL_SECS` | 1800 | Min between DMs |
| `COMPILE_INTERVAL_SECS` | 1800 | Scheduler tick |
| `NOTIFY_ON_COMPILE` | false | Avoid compile-time spam |

---

## 12. Success metrics by phase

| Phase | Metric |
|-------|--------|
| M4 | Human sees real GitHub → ledger → Slack without spam |
| M5 | Stranger can open HTTPS URL, connect Slack+GitHub, get ≤1 DM / window |
| M6 | Multi-person team digests; conflict/blocker surface; ready for design partners |
| M6.5 | 10–14d learning window complete; veto rate + gold pairs exported |
| M7 | Customer can run local inference SKU; self-serve tenant path |
| Intent | Conflicts surfaced with evidence before status theater |

---

## 13. Document control

| Field | Value |
|-------|--------|
| Ground truth for | Post-M4 product planning |
| Interaction log | `Interaction Log_ Product Decisions.md` (same folder) |
| Change process | Edit this file + ADR if thesis shifts |

*This file is the backlog spine. Implementation tickets should reference section numbers.*
