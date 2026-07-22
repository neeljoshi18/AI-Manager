# Plan: Demo → Product (M5 focus) + Intent/Agents clarity + Plans folder

**Date:** 2026-07-23  
**Mode:** Plan only (no implementation this turn)  
**Focus:** Working demo → real product UX + deploy path; deep answers on intent + agentic layer  
**Ground truth already in repo:**  
- `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md`  
- `starting-out-documents/Interaction Log_ Product Decisions.md`  
- ADRs 011–015  

---

## 0. Immediate ops note (why V1/V2 showed down)

You killed long-running tasks — correct for laptop hygiene. Demo console health pills probe:

| Service | Port | Role |
|---------|------|------|
| V1 | 18080 | Ingest |
| V2 | 18082 | Graph |
| V3 | 18083 | Twins + demo UI |
| egress | 18090 | Slack secrets |

If V1/V2 are down, **manual simulate can still use fixtures**, but **live graph path and “full product” feel fail**. Docker + ngrok may survive sleep; processes usually do not. On wake: restart V1–V3, egress, bridge, ngrok as needed. **Not a product bug** — local process lifecycle.

**After this plan is approved:** revive stack when implementing; create `ai-manager/plans/` and copy this plan there as the new convention.

---

## 1. Strategic answer: what we focus on next

**Yes — next while is “demo → product,” not more verticals for their own sake.**

| Priority | Theme | Why |
|----------|--------|-----|
| **1** | **Product shell** (dashboard redesign, one-command start, health that recovers) | Trust + leads + daily use |
| **2** | **M5 staging path** (hosted URL, secrets, Slack OAuth, GitHub App) | Clients can’t use ngrok forever |
| **3** | **Onboarding** (connect tools in UI) | Definition of “teams can use us” |
| **4** | **Intent classification (spec → thin v0)** | Feeds conflict resolver / twin profile |
| **5** | **Agentic monitors (thin)** | On graph deltas, not a Centaur clone |

**Do not** start a full multi-agent OS before (1)–(3). Intent + agents sit **on** a product people can open.

---

## 2. Intent classification — deep answer (what it is)

### 2.1 Problem

Raw events are **facts**:

- `pull_request.opened`  
- `issue.commented`  
- Slack message in `#eng`  

**Intent** is a **claim about purpose**:

- “Ship auth rewrite this sprint”  
- “Blocked waiting on API keys”  
- “Deprioritize feature X”

Status theater happens when **intents are invisible or conflicting** and humans meet to rediscover them.

### 2.2 What “classification” means (not magic LLM spam)

| Stage | Input | Output | Method (v0 → v1) |
|-------|--------|--------|------------------|
| **Extract** | Event + graph neighborhood | Candidate intent span (title, label, body preview) | Rules: PR title verbs, ticket status, labels (`blocked`, `epic:`) |
| **Type** | Candidate | `SHIP` / `BLOCKED` / `EXPLORE` / `REVIEW` / `OPS` / `UNKNOWN` | Rules + small classifier later |
| **Attach** | Type + actor + resource | Intent node + edges `PROPOSES`, `BLOCKS`, `RELATES_TO` | V2 graph extension |
| **Conflict** | Two intents same resource/team | Conflict card | Same owner? same deadline? BLOCKS both ways? |
| **Surface** | Conflict / high-signal intent | Slack (batched) or dashboard | Human gate |

**v0 (deterministic):** label/title heuristics only — no LLM required.  
**v1 (optional LLM via egress):** rewrite summary **from structured fields only** — never invent work items (same as V3 ledger).

### 2.3 Where it lives

```
V1 events ──► V2 graph ──► Intent extractor (worker)
                    │              │
                    │              ▼
                    │         intent nodes / edges
                    ▼
              V3 status ledger (already uses graph)
                    │
                    ▼
         Conflict resolver + dashboard “Focus / Conflicts”
```

Proposed package name when we code it: **`vertical-4-intent`** or worker under V2 — **only after M5 path is usable**. Spec first in `plans/`.

### 2.4 Employee profile / digital twin substrate

| Profile facet | Source |
|---------------|--------|
| Active work | Open PRs/tickets (graph) |
| Outcomes | Merged/closed (High confidence) |
| Stated intent | Bot DM + ticket goals + classified claims |
| Blockers | BLOCKS edges + “blocked” intents |
| Status narrative | V3 ledger snapshots over time |

**Not:** LOC ranks, private 1:1 wiretaps, browser full-page scrapes without opt-in.

---

## 3. Agentic infrastructure — deep answer (what we build)

### 3.1 Thesis constraint (ADR-011)

Agents **watch and draft**. Humans **veto**. We do **not** ship a multiplayer coding OS.

### 3.2 Sub-agents that earn their place

| Agent | Watches | Emits | Human gate |
|-------|---------|-------|------------|
| **Ingest health** | Bus lag, webhook failures | Ops alert | Ops |
| **Graph delta** | New nodes/edges | Intent candidates | Rules first |
| **Status compiler** | Schedule + window | Ledger + optional DM | Already: batch + veto |
| **Conflict detector** | Competing intents | Conflict card in Slack/dashboard | Resolve / dismiss |
| **Negotiator (later)** | Twin A/B team-visible blockers | Proposed plan text | Always before external post |

**Runtime:** Tokio workers + graph API + egress tools — same monorepo pattern as today.  
**Not day-one:** K8s sandbox swarm, Nostr rooms, agent Git forge.

### 3.3 Order vs productization

```
Product shell + deploy  ──►  Intent v0  ──►  Conflict v0  ──►  Richer agents
     (M5)                    (M6)           (M6–M7)         (M7+)
```

---

## 4. UX exploration — redesign dashboard as product

### 4.1 Current gap

Today’s `/demo/` is a **lab console**: health pills, simulate button, raw JSON. Good for us; weak for buyers.

### 4.2 Product UX pillars

1. **Home — “What’s true now”**  
   - Team blockers, open conflicts, last status window  
   - Not “run a test”

2. **My status**  
   - Current draft ledger, Publish / Edit / Veto  
   - Link “why” → evidence (PR/ticket ids)

3. **Connections**  
   - GitHub / Slack / Jira status (connected, last event age)  
   - Replace “V1 down” with “GitHub: last event 2m ago”

4. **People / twin (later)**  
   - Profile cards, not rankings

5. **Admin**  
   - Notify interval, shadow mode, tenant  
   - One-command / deploy health

### 4.3 UX references (steal patterns, not clones)

| Product | Steal |
|---------|--------|
| Linear | Clean hierarchy, keyboard-friendly, calm density |
| Slack | Where delivery lives — don’t force a second chat OS |
| Vercel dashboard | Connector health, “project connected” clarity |
| Notion AI | Summaries with sources/citations feel |
| Geekbot | What to **avoid**: empty nag forms |

### 4.4 Information architecture (proposed product UI)

```
/app
  /today          # org pulse
  /me/status      # personal ledger + actions
  /conflicts      # intent clashes
  /connections    # integrations
  /settings       # cadence, shadow, team channel
/demo             # keep as engineer sandbox OR merge into /app with ?dev=1
```

### 4.5 Visual direction

- Dark calm product chrome (current palette is fine)  
- **Status as first-class objects**, not log lines  
- Hide raw JSON behind “Debug”  
- Health: green when **last successful ingest** recent, not only process up  
- Empty states that teach onboarding (“Connect GitHub”)

### 4.6 UX deliverable in next build phase

Not a full design system yet — a **Product App v1** shell:

- One SPA or server-rendered polish on twin-api  
- Real data from V3 APIs + connection probes  
- Copy: meeting elimination, not “test suite”

---

## 5. Next steps roadmap (sequenced)

### Phase A — Plans hygiene (first implementation commit)

- Create monorepo folder: **`plans/`**  
- Save this plan as e.g. `plans/2026-07-23_demo-to-product-m5.md`  
- Convention: `plans/YYYY-MM-DD_slug.md` + optional `plans/README.md` index  
- Link from root README + Session Handoff  

### Phase B — Local product reliability (1–2 days)

- `scripts/dev_up.sh` / compose profile: start V1+V2+V3+egress+bridge  
- Document wake-from-sleep restart  
- Dashboard connection panel (last event time)  
- Kill zombie-process confusion  

### Phase C — Product UI redesign (3–7 days)

- Redesign shell per §4  
- My Status + Connections + Today  
- Keep demo simulate as “Send test status” under Connections  

### Phase D — M5 staging (1–2 weeks)

- Public URL + TLS  
- Slack OAuth install  
- GitHub App (retire ngrok for real tenants)  
- Secrets story (still egress)  

### Phase E — Onboarding  

- Wizard: create tenant → connect Slack → connect GitHub → shadow mode → first DM  

### Phase F — Intent v0 + agent monitors  

- Spec TAS for intent layer  
- Deterministic classifier + graph edges  
- Conflict cards  
- Sub-agent watchers as workers  

### Phase G — M6 design partner  

- Jira/Linear  
- Slack channel ingest  
- Metrics: standups canceled, veto rate  

---

## 6. “Plans folder” process (from now)

| Rule | Detail |
|------|--------|
| Location | `ai-manager/plans/` (monorepo root) |
| Naming | `YYYY-MM-DD_short-slug.md` |
| Content | Goal, non-goals, decisions, UX notes, phased tasks, open questions |
| After each plan session | Copy/write plan into `plans/` (when not in plan-only mode) |
| Living backlog | Still update `Product Roadmap_…md` for spine; **plans/** are dated snapshots |

**This session:** plan exists in session plan file; **first action after approval** is materializing `plans/`.

---

## 7. What checks out vs risks

### Checks out

- Time-based notify + full ingest = correct product economics  
- Demo → product is the right bottleneck  
- Intent/conflict needs graph + evidence, not chatbots alone  
- Agentic layer as monitors fits ADR-011  
- Private 1:1 DMs stay consent-based (ADR-015)  

### Risks if we ignore order

| Risk | Mitigation |
|------|------------|
| Redesign UI but nobody can deploy | M5 in parallel with UI |
| Intent/LLM before connectors | Rules v0 first |
| Agent sprawl | Max 2–3 workers until partner |
| Scope = browser spyware | Opt-in metadata only |
| Laptop sleep kills demo | Staging + process supervisor |

---

## 8. Suggested first implementation slice (when you exit plan mode)

1. Create `plans/` + save this document  
2. `dev_up` script + short “wake laptop” runbook  
3. Product UI redesign pass (shell + My Status + Connections)  
4. Then M5 staging spike  

**Not first:** full intent classifier, browser extension, multi-agent negotiation.

---

## 9. Open questions (optional — can decide later)

1. Hosting preference for M5: Fly / Render / single VPS?  
2. First ticket system: Jira or Linear?  
3. Keep `/demo` forever or fold into `/app`?  

Default if undecided: VPS or Fly; Linear first (dev-native); fold demo into app as “Lab”.

---

## 10. Success criteria for “product not test suite”

- [ ] Stranger opens URL, understands value in 30s  
- [ ] Connect GitHub + Slack without reading internal READMEs  
- [ ] Gets **at most** one status DM per notify window  
- [ ] Can veto/publish from Slack or UI  
- [ ] Sees connections health as “last event,” not only process up/down  
- [ ] Intent/conflicts appear only after above works  

---

## 11. Document map after approval

| Path | Role |
|------|------|
| `plans/2026-07-23_demo-to-product-m5.md` | This plan (dated) |
| `starting-out-documents/Product Roadmap_….md` | Living backlog spine |
| `starting-out-documents/Interaction Log_….md` | Decision history |
| ADRs 014–015 | Batch notify + private DM policy |

---

*Plan mode: no code executed. Implementation waits for approval.*
