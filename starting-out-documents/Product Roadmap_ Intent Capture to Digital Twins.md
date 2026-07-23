# Product Roadmap — Intent Capture → Digital Twins → Conflict Resolution

**Date:** 2026-07-22  
**Status:** Living plan (post-M4 foundation)  
**Related:** Session Handoff; ADR-006/011/012/013; V1–V3 TAS  
**Purpose:** Single place for **what we build next**, expanded scope, privacy rules, and how it stays on-thesis.

---

## 1. North star (unchanged)

| We are | We are not |
|--------|------------|
| Permissioned **engineering context plane** | Glean-class full-text/vector search |
| **Meeting elimination** via evidence-backed status | Buzz multiplayer workspace / Git forge |
| **Intent capture** for conflict resolution & twins | Centaur multiplayer agent OS (steal egress only) |
| Success: meetings deleted / focus time reclaimed | Engagement rankings / LOC surveillance product |

**Spine (already shipping in code):**

```
Sources → V1 ingest (ACL) → V2 org graph → V3 status twins → Slack (egress, veto-first, batched notify)
```

Everything below **extends this spine**. It does not replace it.

---

## 2. Where we are now (truth)

| Layer | Status |
|-------|--------|
| V1 GitHub webhooks | Live via ngrok + tenant secrets |
| V2 graph projection | Live (HTTP bridge + project API) |
| V3 ledgers + veto + demo console | Live |
| Slack **outbound** DM/channel | Live via egress |
| Batched notify (not 1:1 webhook→DM) | Live (`NOTIFY_INTERVAL_SECS`, etc.) |
| Slack **inbound** (read channels/DMs) | **Not built** (bot scopes may allow later) |
| Jira / Linear / docs / browser | Normalizers partial (Jira etc. in V1); not productized |
| Multi-agent / conflict resolver | Spec only (phase later) |
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

### 7.1 Client onboarding (M5–M7)

1. **Create workspace** (tenant)  
2. **Connect Slack** (OAuth install — bot + optional events)  
3. **Connect GitHub** (GitHub App preferred over manual webhooks)  
4. **Connect Jira/Linear** (OAuth + webhooks)  
5. **Map users** (provider id → global_user_id)  
6. **Shadow mode** 7–10 days (compile only)  
7. **First standup kill** (enable Medium silence / High auto carefully)  
8. **Admin console** (health, connectors, last ledger, ACL audit)

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
- [ ] Single-tenant public host (VM or Fly/Render) + real DNS/TLS  
- [ ] Secrets vault path (not only file)  
- [ ] Slack OAuth install link (wired; manifests exist)  
- [ ] GitHub App production install  
- [x] Health dashboard last-event age + deploy runbooks  


### Phase M6 — Design partner

- [ ] Jira **or** Linear webhook path productized  
- [ ] Slack **channel** message ingest (metadata + short text)  
- [ ] Bot-DM intent capture (“I’m working on X”)  
- [ ] Weekly metrics: standups canceled, DMs sent, veto rate  
- [ ] Conflict detector **v0** (rule-based on BLOCKS + dual owners)  

### Phase M7 — Self-serve + twins depth

- [ ] Multi-tenant control plane  
- [ ] Onboarding wizard  
- [ ] Person profile API (digital twin read model)  
- [ ] Optional browser extension (opt-in host/title)  
- [ ] Sub-agent monitors as first-class workers  
- [ ] Billing / seats (if SaaS)  

### Phase M8+ — Advanced (parked)

- [ ] Enterprise Slack compliance ingest (legal-gated)  
- [ ] Twin-to-twin negotiation  
- [ ] Docs metadata connectors (Notion/Confluence) without full-text index  
- [ ] IDE extension  

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

### Explicitly out / constrained

| Feature | Rule |
|---------|------|
| Silent private DM wiretap | **No** |
| Full doc corpus + vectors | **No** (ADR-006) |
| Individual productivity rankings | **No** |
| Full Centaur sandboxes as product | **No** (ADR-011) |
| Hosting proprietary source | **No** |

---

## 10. Vertical / workstream naming (proposed)

| Name | Role |
|------|------|
| **V1** | Telemetry / connectors / ACL membership |
| **V2** | Org + intent graph |
| **V3** | Status twins / delivery / bot UX |
| **V4 Intent & Conflicts** (new when started) | Intent nodes, conflict resolver, agent monitors — **only after** M5 path stable |
| **vertical-security** | Egress / secrets |
| **Platform** | Deploy, onboarding, multi-tenant |

Do **not** start V4 coding until M5 checklist is mostly green (founder rule).

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
| M5 | Stranger can open URL, connect Slack+GitHub, get a DM |
| M6 | Partner cancels ≥1 standup/week; veto rate measured |
| M7 | Self-serve tenant in <30 minutes |
| Intent | Conflicts surfaced with evidence before status theater |

---

## 13. Document control

| Field | Value |
|-------|--------|
| Ground truth for | Post-M4 product planning |
| Interaction log | `Interaction Log_ Product Decisions.md` (same folder) |
| Change process | Edit this file + ADR if thesis shifts |

*This file is the backlog spine. Implementation tickets should reference section numbers.*
