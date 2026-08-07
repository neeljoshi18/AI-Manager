# Plan: Intent Adequacy Experiment → Real Intent Capture

**Date:** 2026-08-06  
**Staging:** https://status.neel.world/app/  
**Thesis anchor:** [Intent is the new interface](https://neel.world/intent-is-the-new-interface.html)  
**Product spine:** Sources → V1 → V2 graph → V3 twins → Slack delivery (veto-first)  
**Doctrine:** Chat = delivery; GitHub = work; no LOC rankings; no silent 1:1 wiretap; vault for tokens.

---

## 0. What we do now (one-line)

**Run a data-adequacy experiment:** feed our live flywheel (graph + digests + heat + team map) to an agent as if it is a real team, force a **developer-grade employee profile of Neel**, then force a **structured critique of missing signals for intent/conflict**. Use that critique to lock the next engineering wedge: **conflicts from visible friction first**, then **stated intent**, then **channel Slack as context (not Glean)**.

Do **not** rebuild cockpit/sales PDFs. Do **not** open Linear/training. Pipeline packaging is largely solved; **intent fidelity is the bottleneck**.

---

## 1. Philosophical frame (why this is the hard problem)

### 1.1 Your thesis (compressed)

From the essay:

1. **Interface collapse** — Systems today see *actions*, not *meaning*. Intent is the lost compression of “what I meant.”
2. **Noise vs claim** — The hard part is not the model; it is deciding **what counts as intent**. Wrong here → eager helpers → **trust death**.
3. **Primary object** — Winners treat **intent** as the product object, not prompts, chat history, or dashboards.
4. **Preference/mood layer** — High-resolution maps of how people think are powerful and dangerous; sit with that before shipping “AI org brain.”
5. **Reading the room** — Agent tools follow explicit instructions; they don’t yet notice frustration and quietly fix the process.

AI Manager’s product bet is the operational form of this: **permissioned engineering context plane** that turns work exhaust + stated claims into status and conflict surfaces so standups die—not a chat OS, not LOC surveillance.

### 1.2 Intent as an engineering object (developer-evaluator view)

If you evaluate developers (and systems that evaluate them), you already know:

| Layer | What it is | Example | Reliability for “intent” |
|-------|------------|---------|---------------------------|
| **Fact** | Observable event | `push`, `review_requested`, merge conflict file list | High |
| **Trajectory** | Time-ordered facts | Author A commits on auth for 3 days | Medium (purpose inferred) |
| **Claim** | Stated purpose | PR title “ready to ship”; Slack “blocked on security” | High *if* captured |
| **Commitment** | Claim + deadline/owner | “Ship by Friday, owner @neel” | High when explicit |
| **Preference** | Style over time | Prefers small PRs, late-night hours | Soft; dangerous if ranked |
| **Mood / room** | Frustration, urgency | “this is broken again” in #prod | Soft; needs consent + context |

**Intent (for us) should mean:** a **typed claim about purpose attached to work and owner**, with **evidence IDs**, **confidence**, and **supersession**—not “vibe from LOC.”

That maps almost exactly to existing code (`vertical-2/.../intent.rs`):

- Types: `SHIP | BLOCKED | FIX | EXPLORE | REVIEW | FREEZE | OTHER`
- Structure: `IntentClaim { intent_type, summary, about_node_id, owner_node_id, confidence, evidence }`
- Conflicts: `dual_blocks | ship_vs_freeze | dual_owners | open_blocker`

**Philosophical trap:** treating *trajectory* as *claim*. Commits show **what happened**; they rarely show **whether the author still wants to ship vs freeze**. Conflicts between *facts* (merge conflict, CI red) are easier; conflicts between *wills* need claims.

### 1.3 Why conflicts are the on-ramp (you’re right)

Conflicts are **visible friction**:

- Git: merge conflicts, force-push, long-lived PR, review stall, CI red after “LGTM”
- Process: SHIP vs FREEZE labels, draft vs ready, “DO NOT MERGE”
- Social (channel, not private DM): “wait don’t ship”, “blocked on X”, “who owns Y?”

**Conflict is intent made observable by opposition.** Two claims (or a claim vs a fact) collide. That is easier than positive intent (“what is Neel optimizing for this quarter?”) which needs **stated goals** or long temporal aggregation.

**Doctrine-safe order:**

```
Visible friction (conflicts)  →  Stated claims (tickets, PR text, bot DM, channel snippets)
    →  Trajectory reinforcement (commits/reviews matching claims)
    →  Soft preference (only for personal twin, never ranking)
```

Never reverse this order in product: soft preference first → surveillance product → trust death (essay).

### 1.4 “AI organization” bottleneck (honest)

Pipeline solved: ingest → graph → digest → Approve/Don’t send → Neon mirror.

**Bottleneck:** reliable **person-scoped intent graph** such that an agent can answer:

- What is this person *trying to do* this week?
- What is *blocking* them?
- Who *disagrees* (SHIP vs FREEZE)?
- Did they **follow through** on what they said?

Without that, “AI org” is prompt theater. With it, twins can draft status, surface conflicts, and later negotiate—always **human-gated** (ADR-011).

---

## 2. Truth about our data today (adequacy baseline)

Live staging snapshot (2026-08-06):

| Signal | State | Intent quality |
|--------|-------|----------------|
| Multi-repo commits | ~237 Commit nodes, 14 repos, ~10 people on graph | **Trajectory / heat** — strong for “who touches what,” weak for purpose |
| Authors | neeljoshi18 ~197 authored-ish; others thin | **Asymmetry** — founder-heavy flywheel, not a balanced “company” |
| Team map | **2** Slack-mapped person twins (neel + paneerjeera) | Profile experiment can only be serious for mapped people |
| Digests | multi_person_ready; content_people=2 | **Narrative** — good substrate if digests have real bullets |
| Intent nodes | ~6 on graph | Mostly **rules/demo-class** SHIP/FREEZE/BLOCKED on a story PR — **not organic multi-repo intent** |
| Conflict cards | Present in pulse | **Detector works**; evidence often seed-ish / story-1 — good for UI, **bad for claiming real team conflict intelligence** |
| Neon | twin_* continuous + graph_nodes/edges export | SQL ops for experiment dumps |
| Slack **inbound** | **Not built** | Huge gap for “said vs did” |
| Tickets (Linear/Jira) | Deferred | Ticket titles are high-quality intent claims when present |

**Adequacy verdict (pre-experiment hypothesis):**

- Enough to draft a **work profile** (repos, hours, collaborators, commit themes).
- **Not enough** to draft a trustworthy **intent profile** or real **intent conflicts** beyond demo cards.
- The experiment should **prove or falsify** this with a blind agent, not with our own narrative.

---

## 3. Experiment A — Employee profile (Neel) from product data only

### 3.1 Goal

Test: *If we hand an agent only what AI Manager already holds, can it produce a developer-evaluator-grade profile of Neel that a human manager would trust—and can it separate fact vs guess?*

### 3.2 Data pack (export once; no secrets)

Assemble a single JSON/markdown pack (script or manual curl → file). **Exclude** vault tokens, OAuth secrets, raw bot tokens.

Suggested pack fields:

1. **Identity / map**  
   `GET /v3/tenants/ten_github/team` — members, slack maps, twin ids  
2. **Heat / trajectory**  
   `GET …/insights/dev` — by_author, by_day, hour histogram, recent_commits (filter to neeljoshi18)  
3. **Graph neighborhood**  
   Snapshot or Neon: nodes where Person=neeljoshi18 or AUTHORED edges from that person; repos touched; PRs  
4. **Digests**  
   List drafts/ledgers for Neel’s twin (approved + pending + don’t-send if useful) — **this is the narrative gold**  
5. **Pulse / conflicts / intents**  
   Full cards + raw intent list — **label demo vs live** in the pack metadata  
6. **Events (Neon)**  
   Recent twin_events kinds for the tenant (approve/dont_send/compile) — behavior toward status  
7. **Meta honesty block** (written by us, fixed)  
   - Multi-repo poller + webhooks fill graph  
   - Intent/conflict v0 is rules; seed demo may still appear  
   - Only 2 people mapped for digests  
   - No Slack channel read yet  
   - No LOC rankings allowed in conclusions  

### 3.3 Agent brief (system / user prompt)

**Role:** Senior eng manager evaluating a teammate for staffing and risk—not HR surveillance.

**Constraints:**

- Use **only** the pack. No web search inventing resume facts.
- Separate every bullet into: `OBSERVED` | `INFERRED` | `UNKNOWN`.
- No productivity ranking language; no “lazy/hard worker” moralizing.
- Prefer **evidence IDs** (commit sha, PR id, draft id, conflict_id).
- If demo intent/conflict, tag `DEMO_SEED` and do not treat as real org conflict.

**Deliverable schema:**

```yaml
employee_profile:
  subject: neeljoshi18
  as_of: <timestamp>
  role_hypothesis: ...
  current_focus: [{claim, evidence[], confidence}]
  work_surface: {repos[], dominant_themes[], collaborators[]}
  cadence: {peak_hours_utc, peak_days, notes}
  stated_status: {from_digests: [...]}
  intents: [{type, about, evidence, real_or_demo}]
  conflicts_touching: [{id, kind, summary, real_or_demo}]
  follow_through: [{said_or_implied, later_signal, gap}]
  risk_to_team: [...]   # e.g. bus factor, not character
  confidence_overall: 0-1
  what_i_cannot_know: [...]
```

### 3.4 Human scorecard (you grade the agent)

| Criterion | Pass if |
|-----------|---------|
| **Fact hygiene** | ≥80% of strong claims have evidence IDs |
| **Demo hygiene** | Does not treat seed SHIP/FREEZE as live org politics |
| **Intent humility** | Admits unknowns on goals without Slack/tickets |
| **Developer essence** | Captures *how* Neel ships (themes, systems vs content) not just counts |
| **Usefulness** | A stranger could staff Neel onto a problem area after reading |

**Success is not a flattering bio.** Success is **correct humility** + **useful structure**.

### 3.5 Execution options (implementation later)

| Mode | How | Cost |
|------|-----|------|
| **A1 Manual** | You paste pack + prompt into Grok/Claude once | Fastest for this session |
| **A2 Scripted** | `scripts/profile_experiment_pack.py` dumps pack; agent skill/run | Reproducible |
| **A3 In-product** | Lab page “Generate person profile (shadow)” | Later M7 person-profile API |

**Recommendation:** A1 this week to learn; A2 if we want to re-run after Slack ingest.

---

## 4. Experiment B — Gap analysis prompt (what data for real intent/conflicts)

### 4.1 Goal

Given the same pack, force the agent to think like a **product researcher + conflict system designer**, not a bio writer.

### 4.2 Prompt (core)

> You are designing the **intent layer** for an engineering context plane.  
> Intent = typed claim about purpose (SHIP/BLOCKED/FIX/EXPLORE/REVIEW/FREEZE), owner, work target, evidence, confidence—not chat logs as product.  
> Conflicts = collisions of claims or claim-vs-fact (merge friction, freeze vs ship, dual owners).  
>  
> Given ONLY the attached data pack for tenant ten_github:  
> 1. Score each signal class 0–5 for supporting (a) employee intent profiles (b) intent conflicts.  
> 2. List **missing signal classes** ranked by information gain for *intent conflicts first*, then positive intent.  
> 3. For each missing class: capture method, privacy risk, false-positive risk, and whether it is Fact / Claim / Trajectory.  
> 4. Propose a **minimum viable intent graph** for a 6-person eng pod in 14 days (events, edges, human gates).  
> 5. Explicitly reject: silent 1:1 DM wiretap, LOC rankings, full-doc vector Glean, inventing work items.  
> 6. Output JSON: `{scores, missing_ranked, mvp_14d, anti_features, essay_alignment}` where essay_alignment maps each proposal to: interface collapse / trust / primary object / reading the room.

### 4.3 Expected missing signals (our prior—compare to agent)

Ranked for **intent conflicts** (highest gain first):

1. **PR/issue review state + labels + “do not merge” / draft** — claim + freeze signals from GitHub alone  
2. **CI failure after “ready”** — claim vs fact  
3. **Merge conflict / long-lived PR / review stall timers** — friction facts  
4. **Slack channel short text** (team/prod/troubleshoot) with user→person map — social claims  
5. **Bot DM / slash “I’m blocked on X”** — high-consent claims  
6. **Tickets (Linear/Jira)** — explicit goals/blockers  
7. **Digest edits & Don’t send** — correction of system’s inferred narrative (gold for trust)  
8. **Calendar** (title only) — meeting as status context (secondary)

Lower priority / careful: private DMs (compliance only), IDE paths, browser titles.

### 4.4 Output we keep

Write results into `plans/2026-08-06_intent-adequacy-experiment.md` (or similar): profile draft + gap ranking + decisions for M6 residual work.

---

## 5. How we actually capture intent (target design)

### 5.1 Pipeline (already named in roadmap; make it real)

```
extract text/labels from Fact
   → type (rules v0 first; LLM only later for prose, not inventing nodes)
   → attach Intent node + edges (PROPOSES / BLOCKS / ABOUT / OWNED_BY)
   → conflict detect (rules)
   → surface (pulse + optional batched Slack card)
   → human resolve / supersede
```

**Invariant:** Every Intent node has **evidence[]** pointing at graph or event ids. No free-floating “AI thought.”

### 5.2 Three capture tracks (build order)

#### Track C — Conflicts from friction (P0 next)

**Essence:** Developer-visible pain without reading private chat.

| Detector | Inputs | Emits |
|----------|--------|-------|
| Ship vs freeze | Labels, draft/ready, title keywords, branch protection notes | `ship_vs_freeze` |
| Open blocker | BLOCKED intent or “blocked on” in PR body | `open_blocker` |
| Dual owners | Multiple assignees / reviewers with opposing claims | `dual_owners` |
| Merge friction | Conflicted PR, repeated force-push, CI red after approval | new kinds: `merge_friction`, `stale_review` |
| Cross-person blocks | BLOCKS edges between work items | `dual_blocks` |

**Why first:** Matches your instinct; works on GitHub alone; demo can be replaced by **organic** cards as soon as multi-PR graph is dense.

**Adequacy lever:** Multi-repo poller already fills commits; strengthen **PR/review/CI projection** quality (not more commit LOC).

#### Track S — Stated intent (P1)

| Source | Mechanism | Consent |
|--------|-----------|---------|
| PR/issue title+body | Rules extract at project time | Public to collaborators |
| Labels / milestones | Direct type map | Public |
| Bot DM | “I’m working on / blocked on” → IntentClaim | Explicit to bot |
| Slash `/ai-manager capture` | Thread → intent + evidence link | Explicit |
| Ticket webhooks | Linear/Jira title+status | Org install |

**Follow-through loop (said vs did):**

```
Claim C at t0 about work W
  → later Facts F on W (commits, merges, state change)
  → score: supported | contradicted | abandoned | superseded
```

This is the **digital twin substrate** without personality spy features.

#### Track L — Language in Slack channels (P1, careful)

You asked: can we read Slack and map users → developer tags and follow-up?

**Yes, with a bot in the channel—not private 1:1 wiretap.**

| Surface | Feasible? | Product stance |
|---------|-----------|----------------|
| Public/private **channels** where bot is invited | Yes (Events API: `message.channels`, `message.groups`) | **In scope** — team chose shared surface |
| Multi-party DMs with bot | Yes if bot present | Opt-in capture |
| Human↔human 1:1 | **No** normal bot | Out of default product |
| Enterprise Compliance/Discovery | Possible later | Legal SKU only |

**Competition note:** Many tools do full Slack search/Glean-style. **We should not.** Our wedge:

- **Not** “search everything you said.”  
- **Yes** “extract **IntentClaims** and **follow-through** from team channels + attach to graph people/work.”  
- Store: short preview (≤280), channel id, user id, timestamp, link to message permalink—**not** a second chat archive product.  
- Map: Slack user → global_user_id → Person node → twin (already partially there).

**Channel taxonomy (config, not ML day one):**

| Channel class | Intent use |
|---------------|------------|
| `#eng` / general | Soft claims, ownership questions |
| `#prod` / incidents | BLOCKED, FIX, urgency |
| `#deploy` / release | SHIP / FREEZE |
| `#troubleshoot` | BLOCKED + evidence |

**Reading the room (essay) without trust death:**  
Detect *process friction phrases* (“this is broken again”, “who owns…”) → **conflict or blocker candidate**, never auto-email manager with a personality report.

### 5.3 What “essence” means for a developer profile

A good profile answers as a tech lead would:

1. **What systems do they own / orbit?** (repos, paths, PR clusters)  
2. **What are they trying to finish?** (open claims + open PRs)  
3. **What is stuck?** (blockers, review wait, CI)  
4. **Who do they couple with?** (review edges, co-change—not “friends list”)  
5. **Do words match commits?** (follow-through score)  
6. **What is unsafe to infer?** (motivation, hours-as-virtue, private life)

That is **intent-compatible evaluation**, not stack-rank.

---

## 6. Feature backlog (along these lines)

Prioritized for residual M6 → person-profile path. Aligns with existing roadmap; this plan **narrows** after the experiment.

### P0 — This experiment + honesty layer

- [ ] **Data pack exporter** (script) for profile + gap experiments  
- [ ] **Tag demo/seed intents** in API responses (`seed` / `source: rules_demo`) so agents and UI never confuse them  
- [ ] **Person profile shadow endpoint** (read-only JSON; later UI Lab) — structure only, no rankings  
- [ ] Write-up of experiment results → update intent roadmap decisions  

### P0/P1 — Conflict reality (GitHub-native)

- [ ] Organic intent attach on real PR/issue project (not only seed)  
- [ ] New conflict kinds: stale review, merge_friction, CI-after-ready  
- [ ] Pulse: split **live vs demo** conflicts (partially done earlier—verify still true)  
- [ ] Surface one **real** conflict card on multi-repo pilot data  

### P1 — Slack channel intent (anti-Glean)

- [ ] Slack Events subscription for channels bot is in  
- [ ] V1 normalizer: `chat.message` with ACL = channel membership  
- [ ] Rules extract IntentClaim from short text; attach to Person + optional work link  
- [ ] **Follow-through job**: claim → later GitHub facts  
- [ ] Champion setting: which channels are “status-truth” surfaces  
- [ ] Explicitly **no** full-history search UI v1  

### P1 — Stated intent UX

- [ ] Bot DM parse: blocked/working-on  
- [ ] Slash capture  
- [ ] IC can see “your open intents” and supersede/veto them (same trust pattern as digests)  

### P2 — Tickets + CI

- [ ] Linear or Jira webhook productized (tickets as claims)  
- [ ] Actions/check suite → ship confidence / blocker  

### P2 — AI org substrate (only after claims work)

- [ ] Person profile API as twin read model (roadmap M7)  
- [ ] Conflict resolver cards with human resolve path in Slack  
- [ ] Optional Model Router for **prose only** over structured claims (ADR-016)—never invent work  

### Explicit non-features (keep listing)

- Silent 1:1 Slack wiretap  
- LOC / productivity rankings  
- Full Slack/doc vector search as core product  
- Auto-acting on half-formed intent (essay: trust death)  
- Training on raw source dumps before gold structured pairs  

---

## 7. Path to “map employee intent → AI organization”

```
Now: flywheel + digests + graph + demo intent/conflict + Neon
  │
  ├─ Exp A/B: prove data inadequacy honestly
  │
  ▼
Conflicts real (GitHub friction + rules)     ← trust-building, visible
  │
  ▼
Stated claims (PR text, bot, tickets) + follow-through
  │
  ▼
Channel Slack snippets → claims (not archive)
  │
  ▼
Person profile = twin read model (facts + claims + conflicts + follow-through)
  │
  ▼
Agents draft status / conflict cards (human veto)  ← “AI org” ops layer
  │
  ▼
Later: twin-to-twin negotiation proposals (still gated)
```

**Definition of done for “intent bottleneck unlocked” (pilot):**

1. For each mapped engineer: ≥1 **organic** open intent or explicit “no active claim.”  
2. At least one **non-demo** conflict card per week of real team work *or* a clean empty state with reason.  
3. Follow-through score computable for claims older than 72h.  
4. Profile experiment re-run scores **pass** fact hygiene + intent humility.  
5. Champion can explain a conflict without opening a standup.

---

## 8. Execution plan for next session(s)

### Session N (experiment — can be same week)

1. Export data pack (script or curated curls) for `ten_github`, subject `neeljoshi18`.  
2. Run **Experiment A** (profile) with the prompt in §3.3.  
3. Run **Experiment B** (gap analysis) with §4.2.  
4. Human scorecard §3.4; write `plans/2026-08-06_intent-adequacy-experiment.md`.  
5. Decision freeze: top 3 missing signals to implement first (default proposal below if agent agrees).

**Default post-experiment build order (unless agent strongly revises):**

1. Organic PR/issue intent + friction conflicts (no Slack required)  
2. Demo-vs-live tagging everywhere  
3. Slack channel Events → short IntentClaims + person map  
4. Follow-through job  
5. Person profile shadow API  

### Session N+1 (engineering wedge)

Implement (1)+(2); re-run pack export; show one live conflict on staging cockpit.

### Session N+2

Slack channel ingest (bot already install path); champion channel picker; privacy copy.

---

## 9. Experiment prompts (copy-paste ready)

### 9.1 Profile prompt (user message after attaching pack)

```
You are a senior engineering manager. Using ONLY the attached AI Manager data pack
for tenant ten_github, produce an employee_profile YAML for subject neeljoshi18.

Rules:
- Tag every bullet OBSERVED | INFERRED | UNKNOWN
- Cite evidence ids (commit, PR, draft, conflict_id) when OBSERVED
- Tag DEMO_SEED anything that looks like intent_demo / story seed
- No LOC rankings, no character judgments, no inventing employers/schools
- Separate work trajectory from stated intent

Then rate your own confidence 0-1 and list the 5 questions you most need
answered by new data sources (prefer conflict-relevant questions).
```

### 9.2 Gap prompt (user message)

```
Using ONLY the same pack: design the intent layer for this product.
Intent = typed purpose claims with owners and evidence.
Conflict = claim/claim or claim/fact collision.

Return JSON with:
scores: {signal_name: {intent_profile:0-5, intent_conflict:0-5, notes}}
missing_ranked: [{signal, why, capture, privacy, false_positive_risk, class: fact|claim|trajectory}]
mvp_14d: {events, edges, human_gates, success_metrics}
anti_features: [...]
essay_alignment: {interface_collapse, trust, primary_object, reading_the_room}

Prefer conflict-visible signals over soft preference. Reject silent DM wiretap and Glean-style archive.
```

---

## 10. Risks

| Risk | Mitigation |
|------|------------|
| Agent confuses demo conflicts with reality | Pack meta + DEMO_SEED rule + code tags |
| Profile feels like surveillance | Manager-eval framing; no rankings; UNKNOWN required |
| Slack channel read becomes “we store all chat” | Cap preview length; intent extract only; no search product v1 |
| Eager auto-actions on inferred intent | Human veto always; essay trust rule |
| Founder-heavy graph skews “company” experiment | State asymmetry in pack; later re-run with fuller pod map |

---

## 11. Immediate next action after plan approval

1. Build or manually assemble the **data pack** for Neel.  
2. Run Experiment A + B (agent).  
3. Score and write results plan file.  
4. Lock engineering order (default §8).  
5. Only then implement organic conflicts / Slack Events—not before learning what the agent says is missing.

---

## 12. Summary

| Question | Answer |
|----------|--------|
| What now? | Adequacy experiment on live data + deep intent roadmap freeze |
| Is data enough for a real profile? | **Trajectory yes; intent/conflicts mostly no** (hypothesis to test) |
| Intent vs conflict? | **Conflicts first** (visible friction); intent claims next |
| Slack read? | **Channels yes** (bot member); **not** private 1:1; anti-Glean extract-only |
| AI org? | Unblocked only when person-scoped claims + follow-through + human-gated agents exist |

The product pipeline is solved enough that **the only strategic game left is making intent a trustworthy primary object**—exactly as the essay argues—without becoming the surveillance tools that kill trust.
