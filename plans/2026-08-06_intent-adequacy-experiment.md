# Intent Adequacy Experiment — Results

**Date:** 2026-08-06 (pack fetched 2026-08-07T06:00:23Z)  
**Tenant:** `ten_github`  
**Subject:** `neeljoshi18`  
**Staging:** https://status.neel.world  
**Pack:** [`plans/packs/2026-08-06_ten_github_neeljoshi18.json`](packs/2026-08-06_ten_github_neeljoshi18.json)  
**Exporter:** `scripts/intent_adequacy_pack.py`  
**Plan anchor:** [`plans/2026-08-06_intent-capture-and-adequacy.md`](2026-08-06_intent-capture-and-adequacy.md)  
**Thesis:** [Intent is the new interface](https://neel.world/intent-is-the-new-interface.html)

---

## 0. Executive verdict

| Question | Answer |
|----------|--------|
| Can we draft a **work / trajectory profile** of Neel from product data alone? | **Yes** — strong. Repos, commit themes, cadence, collaborators are real. |
| Can we draft a trustworthy **intent profile** (what he is *trying* to do, with claims)? | **No** — not beyond trajectory inference. All typed Intent nodes are demo seeds. |
| Can we surface **real intent conflicts** for a team? | **No** — conflict engine works, but every card in the pack is story-1 / demo-seed. |
| Adequacy for “AI org” person-scoped intent graph? | **Trajectory yes; claims/conflicts no.** Pipeline packaging is ahead of intent fidelity. |

**One-line:** The flywheel proves *who touches what*; it does not yet prove *what people meant* or *who disagrees*—except via seeded theater on `pr:…/story-1`.

---

## 1. Pack stats (observed)

| Metric | Value |
|--------|------:|
| Graph nodes (insights snapshot) | 268 |
| Graph edges (insights) | 501 |
| Commit nodes | **237** |
| Person nodes | **10** |
| Repo nodes | **14** |
| Intent nodes | **6** (all seed-tagged) |
| PullRequest nodes | **1** (`story-1` demo PR) |
| AUTHORED edges (activity) | 242 |
| `neeljoshi18` authored | **197** (~81%) |
| Next authors | syon 11 · Adityakushwaha2006 6 · Syon-Pratap 6 · Copilot 6 · paneerjeera 5 · … |
| Mapped person twins (Slack) | **2** (neeljoshi18, paneerjeera) |
| Digests with content | 2/2 mapped |
| Pulse conflict cards | 5 primary (+ demo_cards 2); **5/5 demo-tagged** in pack |
| Conflicts proxy | 7; **7/7 demo-tagged** |
| Pulse intents sample | 6; **6/6** `properties.seed = graph_story` |
| Twin events (last 50) | all `sync_graph_to_db` (ops mirror, not approve/edit trail) |
| Soft outreach ready | true |
| Pack errors | 0 |
| Pack size | ~232 KB |

**Honesty block (from pack meta — fixed doctrine):**

- Multi-repo poller fills graph  
- Intent/conflict may include demo seeds  
- Only mapped people get digests  
- No private Slack wiretap  
- No LOC rankings allowed in conclusions  

---

## 2. Experiment A — Employee profile: `neeljoshi18`

**Role of agent:** Senior eng manager evaluating a teammate for staffing and risk — not HR surveillance.  
**Rules:** OBSERVED | INFERRED | UNKNOWN; evidence IDs; DEMO_SEED tags; no LOC virtue; no invented resume facts.

```yaml
employee_profile:
  subject: neeljoshi18
  as_of: "2026-08-07T06:00:23+00:00"
  twin_id: "twin:person:gu_ec3cab86-2a3c-4737-bb04-d1f2deeae9f8"
  subject_id: "gu_ec3cab86-2a3c-4737-bb04-d1f2deeae9f8"
  person_node_id: "person:gu_ec3cab86-2a3c-4737-bb04-d1f2deeae9f8"
  slack_user_id: "U0APK7W1X99"   # OBSERVED via team map
  role_on_tenant: champion       # OBSERVED team.members[].role

  role_hypothesis: |
    OBSERVED: Champion + sole dense author on ten_github multi-repo graph;
    owns AI-Manager platform surface (digests, graph, Neon, deploy recovery).
    INFERRED: Founder/operator of the pilot tenant rather than IC on a balanced
    6-person pod — author share ~197/242 makes "peer comparison" meaningless.
    UNKNOWN: Job title, company org chart, report chain (not in pack).

  current_focus:
    - claim: "Neon continuous twin mirror + graph export durability"
      evidence:
        - "commit:neeljoshi18/AI-Manager:21dceb0f685f7653852e81b06c8b1ebd94dd6e2a"
        - "commit:neeljoshi18/AI-Manager:204d4c6b2702a9e9163265d48cf70b6ff7c231b7"
        - "dft_8e00150d-2082-49ba-9792-431614e7cf3a"  # published digest bullets
      confidence: 0.85
      tag: OBSERVED  # commit messages + digest items, not a typed IntentClaim

    - claim: "Staging recoverability (V1 force-recreate, deploy paths)"
      evidence:
        - "commit:neeljoshi18/AI-Manager:976cf59cb4c1e9b60d91c0b1c0851277b16a7050"
      confidence: 0.8
      tag: OBSERVED

    - claim: "Product narrative / thesis publishing (intent essay on personal site)"
      evidence:
        - "commit:neeljoshi18/personal-website:708b04874903f956f67547a095b6767a2a294547"
      confidence: 0.75
      tag: OBSERVED

    - claim: "SHIP pilot / dual-owner review on story-1 PR"
      evidence:
        - "pr:neeljoshi18/AI-Manager/pr/story-1"
        - "intent:…:pr:…/story-1:ship"
      confidence: 0.15
      tag: DEMO_SEED  # properties.seed=graph_story; not organic PR intent

  work_surface:
    repos_observed:
      # From subject neighborhood + commit resource paths in pack
      - neeljoshi18/AI-Manager          # dominant platform work
      - neeljoshi18/personal-website
      - neeljoshi18/HateHub
      - neeljoshi18/SAiDL-Summer-Assignment2026
      - neeljoshi18/easemoji
      # Also present on tenant graph (14 repos total) — co-activity / poller scope:
      # raahAI-in/raahAI-ios-app, vibhumaggarwal/raah.ai, Syon-Pratap/Zoom-Attention-Check, …
    dominant_themes:
      - platform_build / verticals (V1–V3, twins, graph)
      - deploy_ops + reliability recover paths
      - digests / Slack delivery / multi-person team map
      - Neon observe mirror (events + graph export)
      - docs/session handoffs
    collaborators_on_graph:
      # AUTHORED counts — trajectory cohabitants, not "friends"
      - paneerjeera (mapped twin, 5 authored; partner on demo story intents)
      - syon / Syon-Pratap (11+6)
      - Adityakushwaha2006, Copilot, Krish-Sachdev-7, LordVantablack, vibhumaggarwal, aaryankushwah
    note: |
      OBSERVED: 10 Person nodes, 2 Slack-mapped for digests.
      INFERRED: "Team" for status delivery is 2 people; graph people include
      external collaborators/students/bots without twin mapping.

  cadence:
    peak_hours_utc: 18   # OBSERVED activity.hour_of_day_utc.peak_hour_utc (60 events)
    secondary_hours_utc: [12, 17, 19, 4, 5]  # OBSERVED histogram shape
    peak_days: [Mon, Thu]  # Mon 112, Thu 108 edge timestamps
    recent_day_activity:   # OBSERVED by_day (tenant-wide edge currency)
      "2026-07-30": 42
      "2026-08-03": 34
      "2026-08-05": 39
      "2026-08-06": 24
    notes: |
      OBSERVED: Tenant heat is founder-dominated; cadence ≈ Neel's cadence.
      UNKNOWN: Local timezone, PTO, meetings — no calendar.
      DO NOT: treat night hours as virtue or risk score.

  stated_status:
    from_digests:
      - draft_id: dft_8e00150d-2082-49ba-9792-431614e7cf3a
        status: published
        status_label: shared
        dm_sent: true
        rollup: "blocked — needs attention"   # driven by demo BLOCKS edges
        lookback: "7d (07-30 → 08-06 UTC)"
        real_bullets:
          - "Commit: Fix neon mirror debug log types for compile"
          - "Commit: Neon full twin mirror: events + twins/drafts/maps + sync_to_db"
          - "Commit: Recover: always force-recreate V1 (hangs after soft restart)"
        demo_contaminated_bullets:
          - "Blocker: Blocked via BLOCKS" ×3  # evidence event:story:blocks*
        ledger_id: led_8ea10a869bc4e1b5afbbcf55
        ledger_items_observed: 9
        confidence_rollup: blocker  # DEMO_SEED-contaminated
    note: |
      Digests are the narrative gold for *commits*, but open_blockers and
      SHIP/FREEZE PR lines currently recycle graph_story seeds. A manager
      reading the DM would believe Neel is blocked on partner review — that
      is product theater until organic PR/intent lands.

  intents:
    - type: BLOCKED
      about: pr:neeljoshi18/AI-Manager/pr/story-1
      evidence: intent:person:gu_ec3cab86-…:story-1:blocked
      real_or_demo: DEMO_SEED  # seed: graph_story, conf 0.85
    - type: FREEZE
      about: pr:neeljoshi18/AI-Manager/pr/story-1
      evidence: intent:person:gu_ec3cab86-…:story-1:freeze
      real_or_demo: DEMO_SEED
    - type: SHIP
      about: pr:neeljoshi18/AI-Manager/pr/story-1
      evidence: intent:person:gu_ec3cab86-…:story-1:ship
      real_or_demo: DEMO_SEED
    organic_open_intents: []   # OBSERVED empty
    note: |
      Same three types also exist for paneerjeera on the same PR (dual-owner demo).
      Pulse demo_count=3; all six Intent nodes are story seeds.

  conflicts_touching:
    - id: cfl_ten_github_pr:neeljoshi18/AI-Manager/pr/story-1_owners
      kind: ship_vs_freeze
      summary: SHIP vs FREEZE on story-1
      real_or_demo: DEMO_SEED
    - id: cfl_ten_github_blocks_ship_intent:…:story-1:blocked_…
      kind: dual_blocks
      real_or_demo: DEMO_SEED
    - id: cfl_ten_github_blocked_intent:…:story-1:blocked
      kind: open_blocker
      summary: "BLOCKED: waiting on partner review"
      real_or_demo: DEMO_SEED
    organic_live_conflicts: []  # OBSERVED — pack tags 5/5 pulse cards demo

  follow_through:
    - said_or_implied: "Digest claims blocked state (BLOCKED via BLOCKS)"
      later_signal: "Continued AI-Manager + personal-website commits through 2026-08-06"
      gap: |
        DEMO_SEED said-vs-did is uninterpretable: blockers are story edges
        (event:story:blocks), not human claims. Cannot score follow-through.
      tag: UNKNOWN for real follow-through
    - said_or_implied: "Commit messages imply Neon mirror 'done' work"
      later_signal: "Subsequent fix commit on neon mirror debug log types"
      gap: "Trajectory refinement, not claim supersession — no Intent node for Neon work"
      tag: INFERRED

  risk_to_team:
    - kind: bus_factor
      claim: "Almost all graph trajectory is one author (197/242)"
      evidence: insights_dev.activity.by_author
      tag: OBSERVED
      note: Staffing risk if this tenant were sold as a multi-IC pilot without more maps.
    - kind: demo_contamination
      claim: "Status digests escalate 'blocked' from seed BLOCKS edges"
      evidence: ledger open_blockers + event:story:blocks
      tag: OBSERVED
      note: Trust risk if a stranger pilot sees fake blockers.
    - kind: pr_surface_thin
      claim: "1 PullRequest node vs 237 commits — friction detectors have almost nothing organic to chew"
      evidence: insights_dev.graph.by_type
      tag: OBSERVED

  confidence_overall: 0.55
  # High on work surface/cadence; near-zero on real intent/conflicts.

  what_i_cannot_know:
    - Quarter goals / roadmap priorities (no tickets, no stated IntentClaims)
    - Whether story-1 SHIP/FREEZE reflects any real disagreement (it does not)
    - Review latency, CI health, merge conflicts on real PRs (PR projection thin)
    - Slack channel claims ("blocked on X", "don't ship") — inbound not built
    - Digest edit / Don't-send corrections as preference signal (events log is ops-only here)
    - Calendar / on-call / meeting load
    - Motivation, hours-as-virtue, private life (out of doctrine anyway)

  questions_for_new_data:  # prefer conflict-relevant
    1. Which real open PRs have draft/ready, labels, or "DO NOT MERGE"?
    2. Any CI red after ready-for-review in the last 7d?
    3. Any merge-conflicted or >N-day review-stalled PRs?
    4. Did Neel (or partner) state a blocker in a team channel the bot is in?
    5. After a human claim, did later commits/merges support or abandon it?
```

### 2.1 Manager read (plain language)

If I were staffing Neel onto a problem area from this pack alone:

- **Put him on:** platform reliability, multi-tenant observe/mirror, digest pipeline, deploy recovery — that is what the commit texture *is*. Evidence: Neon/deploy/graph/twin themes dominate subject commit samples; digest ledger lists those commits with node IDs.
- **Do not put him on:** “resolve the ship vs freeze conflict with paneerjeera on story-1” — that is a **demo fixture**, not org politics.
- **Team context:** Only two humans get digests. The other eight Person nodes are graph cohabitants (collaborators/bots), not status peers.
- **Asymmetry warning:** Author concentration means heat maps and “team” digests currently narrate a founder monorepo more than a balanced eng pod. That is fine for dogfood; it is dishonest for sales if framed as multi-IC intent intelligence.

### 2.2 Human scorecard (Experiment A)

| Criterion | Grade | Notes |
|-----------|-------|-------|
| **Fact hygiene** | **PASS** | Strong claims cite commit node ids, draft id, author histogram, conflict ids. |
| **Demo hygiene** | **PASS** | All 6 intents + all conflict cards tagged DEMO_SEED; not treated as live politics. |
| **Intent humility** | **PASS** | Explicit empty organic intents; confidence 0.55; long cannot-know list. |
| **Developer essence** | **PASS** | Themes (Neon, recover, digests, graph) not raw counts-as-virtue. |
| **Usefulness** | **PARTIAL** | Staffing on systems area works; staffing on *this week's goals* does not. |

**Success definition met:** correct humility + useful structure — not a flattering bio.

---

## 3. Experiment B — Gap analysis (intent / conflicts)

Agent stance: product researcher + conflict system designer. Intent = typed claim (SHIP|BLOCKED|FIX|EXPLORE|REVIEW|FREEZE) with owner, about, evidence, confidence. Conflicts = claim↔claim or claim↔fact collisions.

### 3.1 Scores (0–5)

| Signal class | Employee intent profiles | Intent conflicts | Notes from pack |
|--------------|-------------------------:|-----------------:|-----------------|
| Multi-repo commits / AUTHORED heat | 4 | 1 | 237 commits, 14 repos, 10 people — excellent trajectory; almost no purpose type |
| Person map + Slack twin map | 3 | 1 | 2 mapped — profile serious only for them; conflicts need multi-owner claims |
| Digests / ledgers | 3 | 1 | Narrative gold for commits; blockers polluted by story BLOCKS |
| Typed Intent nodes | 1 | 2 | Schema works; 6/6 seeds — detector demo, not organic |
| Conflict cards (rules_v0) | 1 | 2 | Engine emits dual_blocks / ship_vs_freeze / open_blocker — all story-1 |
| PullRequest / review graph | 1 | 1 | **1 PR node** — starvation for friction detectors |
| CI / check suites | 0 | 0 | Not in pack |
| Slack channel text | 0 | 0 | Inbound not built; oauth shows delivery only |
| Tickets (Linear/Jira) | 0 | 0 | Deferred |
| Twin events (approve/edit/dont_send) | 1 | 0 | 50/50 `sync_graph_to_db` — no human correction trail in sample |
| Calendar | 0 | 0 | Absent |
| OAuth install readiness | 1 | 0 | Booleans only; not intent signal |

### 3.2 JSON-ish deliverable

```json
{
  "scores": {
    "commits_trajectory": {"profile": 4, "conflicts": 1},
    "person_slack_map": {"profile": 3, "conflicts": 1},
    "digests_ledgers": {"profile": 3, "conflicts": 1},
    "typed_intents": {"profile": 1, "conflicts": 2},
    "conflict_cards": {"profile": 1, "conflicts": 2},
    "pr_review_graph": {"profile": 1, "conflicts": 1},
    "ci_checks": {"profile": 0, "conflicts": 0},
    "slack_channels": {"profile": 0, "conflicts": 0},
    "tickets": {"profile": 0, "conflicts": 0},
    "digest_human_corrections": {"profile": 1, "conflicts": 0},
    "calendar": {"profile": 0, "conflicts": 0}
  },

  "missing_ranked": [
    {
      "rank": 1,
      "signal": "Organic PR/issue projection: state, draft/ready, labels, title/body, reviewers, do-not-merge",
      "why_first": "Pack has 237 commits vs 1 PR. Conflict kinds (ship_vs_freeze, open_blocker) starve without real PR nodes. Highest information gain for claim+friction on GitHub alone.",
      "capture": "Webhook + poller project PR/issue nodes and review edges; rules extract IntentClaim from labels/title keywords",
      "privacy_risk": "low (collaborator-visible GitHub)",
      "false_positive_risk": "medium (title keywords mis-type EXPLORE as SHIP)",
      "layer": "Fact + Claim"
    },
    {
      "rank": 2,
      "signal": "Merge friction facts: conflicted PR, force-push bursts, long-lived open PR, review stall timers",
      "why": "Conflict is intent made visible by opposition — does not require Slack",
      "capture": "Graph timers + GitHub PR fields → merge_friction / stale_review kinds",
      "privacy_risk": "low",
      "false_positive_risk": "low-medium",
      "layer": "Fact"
    },
    {
      "rank": 3,
      "signal": "CI failure after ready / after approval",
      "why": "Classic claim-vs-fact; feeds ship confidence and open_blocker",
      "capture": "check_suite / workflow_run → edges on PR nodes",
      "privacy_risk": "low",
      "false_positive_risk": "medium (flaky CI)",
      "layer": "Fact"
    },
    {
      "rank": 4,
      "signal": "Demo-vs-live tagging everywhere (API + pulse + digests)",
      "why": "Pack proves contamination: digest 'blocked' from event:story:blocks. Without tags, agents and champions trust theater.",
      "capture": "Propagate seed/source on Intent + Conflict + ledger items; UI split live vs demo",
      "privacy_risk": "none",
      "false_positive_risk": "low",
      "layer": "Meta / product hygiene"
    },
    {
      "rank": 5,
      "signal": "Slack channel short claims (bot-invited channels only)",
      "why": "Social SHIP/FREEZE/BLOCKED language; person map already exists for 2 users",
      "capture": "Events API message.channels → IntentClaim ≤280 char preview + permalink",
      "privacy_risk": "medium (channel content) — still far below 1:1 wiretap",
      "false_positive_risk": "high without tight rules + human supersession",
      "layer": "Claim"
    },
    {
      "rank": 6,
      "signal": "Bot DM / slash 'I'm blocked on X' / 'working on Y'",
      "why": "High-consent claims; perfect twin substrate",
      "capture": "Interactive message or slash → Intent node OWNED_BY person",
      "privacy_risk": "low (explicit)",
      "false_positive_risk": "low",
      "layer": "Claim"
    },
    {
      "rank": 7,
      "signal": "Follow-through job: claim at t0 → facts on about_node later",
      "why": "Said vs did is the digital twin; pack cannot score it today",
      "capture": "Batch job: supported | contradicted | abandoned | superseded",
      "privacy_risk": "low",
      "false_positive_risk": "medium (need claim quality first)",
      "layer": "Trajectory over Claim"
    },
    {
      "rank": 8,
      "signal": "Digest edits + Don't send as correction signal",
      "why": "Human veto is gold for trust and model later; events sample lacks these kinds",
      "capture": "Persist edit diffs / silence reasons on twin_events; include in pack",
      "privacy_risk": "low (user's own status)",
      "false_positive_risk": "low",
      "layer": "Claim correction"
    },
    {
      "rank": 9,
      "signal": "Tickets (Linear/Jira) titles + status",
      "why": "Explicit goals/blockers when eng pod actually uses them",
      "capture": "Webhook → IntentClaim",
      "privacy_risk": "low-medium",
      "false_positive_risk": "low",
      "layer": "Claim"
    },
    {
      "rank": 10,
      "signal": "Calendar titles only",
      "why": "Secondary status context",
      "capture": "OAuth calendar readonly titles",
      "privacy_risk": "medium",
      "false_positive_risk": "high",
      "layer": "Trajectory soft"
    }
  ],

  "mvp_14d": {
    "goal": "One real non-demo conflict card/week OR honest empty state; organic intents on real PRs for mapped people",
    "events_to_project": [
      "pull_request opened/synchronize/ready_for_review/closed",
      "pull_request_review submitted",
      "issues labeled (blocked, do-not-merge, freeze)",
      "check_suite completed (optional stretch)"
    ],
    "graph_additions": [
      "PR nodes beyond story-1",
      "Intent nodes from labels/title rules with source=github_rules (not seed)",
      "Conflict kinds: merge_friction, stale_review; keep dual_blocks/ship_vs_freeze"
    ],
    "human_gates": [
      "Pulse marks live vs demo",
      "Digest blockers exclude seed edges unless champion opts into demo mode",
      "IC can supersede/veto own intents (same pattern as digests)"
    ],
    "success_metrics": [
      "Intent nodes with seed=null > 0 for ten_github",
      "Pulse conflicts with _demo_seed=false ≥ 1 OR empty_state reason=no_friction",
      "Pack re-run: demo_tagged pulse_conflicts_demo_tagged < pulse_conflicts"
    ],
    "non_goals_14d": [
      "Full Slack archive search",
      "LOC leaderboards",
      "Auto-act on half-formed intent",
      "Person profile UI polish (API shadow ok)"
    ]
  },

  "anti_features": [
    "Silent 1:1 Slack DM wiretap",
    "LOC / commit-count productivity rankings",
    "Full-doc or full-Slack vector Glean as core product",
    "Inventing work items or goals not grounded in evidence ids",
    "Treating demo SHIP/FREEZE as org politics in sales or agents",
    "Auto-emailing managers personality/mood reports",
    "Training on raw dumps before structured claim pairs"
  ],

  "essay_alignment": {
    "interface_collapse": {
      "problem": "Systems see actions (commits) not meaning (claims)",
      "our_move": "Keep commits as trajectory; add typed IntentClaim as first-class graph object with evidence[] — already modeled in vertical-2 intent.rs, underfed by organic extractors"
    },
    "noise_vs_claim_trust": {
      "problem": "Wrong definition of intent → eager helpers → trust death",
      "our_move": "Rank conflicts from visible friction first; require evidence ids; human veto; never rank people by hours/LOC"
    },
    "primary_object": {
      "problem": "Chat history is not the product",
      "our_move": "Intent + conflict cards + follow-through scores are the product objects; Slack is delivery (and later short claim capture), not the archive"
    },
    "preference_mood_danger": {
      "problem": "High-res mind maps are powerful and dangerous",
      "our_move": "Soft preference only on personal twin later; out of 14d MVP; no mood scores in pack conclusions"
    },
    "reading_the_room": {
      "problem": "Agents follow instructions; miss process frustration",
      "our_move": "Channel phrases → blocker/conflict *candidates* with human gate — not manager snitch reports"
    }
  }
}
```

### 3.3 Comparison to plan priors

Plan §4.3 expected missing signals (conflict-first). Agent ranking **agrees** with the plan’s top of list:

| Plan prior | Experiment B rank | Verdict |
|------------|-------------------|---------|
| PR/issue review state + labels + DNM | #1 | **Confirm** — PR starvation is the smoking gun (1 vs 237) |
| CI after ready | #3 | Confirm (after merge friction) |
| Merge conflict / long-lived PR / stall | #2 | Confirm |
| Slack channel short text | #5 | Confirm after GitHub-native |
| Bot DM / slash | #6 | Confirm |
| Tickets | #9 | Confirm deferred |
| Digest edits / Don’t send | #8 | Confirm; pack events currently useless for this |
| Calendar | #10 | Confirm secondary |

**Additional finding the plan underweighted:** **demo contamination of digests** (ledger open_blockers from `event:story:blocks`) is not just a UI cosmetics issue — it falsifies the only “stated status” surface champions see. Promote **demo-vs-live tagging + seed exclusion from blocker rollup** to P0 alongside organic PR projection.

---

## 4. Scorecard (meta — agent / writeup quality)

| Lens | Grade | Comment |
|------|-------|---------|
| Fact hygiene | **A** | Numbers from pack: 237 commits, 197 authored, 14 repos, 10 people, 2 mapped, 6 intents, 1 PR |
| Demo hygiene | **A** | Refused to treat story-1 SHIP/FREEZE/BLOCKED as real; called out digest pollution |
| Intent humility | **A** | confidence_overall 0.55; organic intents empty; follow-through UNKNOWN |
| Deep product thinking | **A-** | Build order tied to PR starvation + trust death, not generic “add AI” |
| Risk of overclaim | Controlled | Sales smoke soft_outreach_ready=true is packaging-ready, not intent-ready |

---

## 5. Recommended build order (decision freeze)

Plan default (§8) vs experiment:

| # | Plan default | Experiment revision | Keep? |
|---|--------------|---------------------|-------|
| 1 | Organic PR/issue intent + friction conflicts | **Confirmed P0** — unblock 1-PR starvation; rules extract on real nodes | **Keep** |
| 2 | Demo-vs-live tagging everywhere | **Elevate:** also strip seed BLOCKS from digest blocker rollup / confidence_rollup | **Keep + sharpen** |
| 3 | Slack channel Events → short IntentClaims | After GitHub friction works; person map ready for 2 users | **Keep as P1** |
| 4 | Follow-through job | Needs organic claims first — else scores demo noise | **Keep order** |
| 5 | Person profile shadow API | Schema from Experiment A is ready; wait until intents aren’t 100% seed | **Keep but gate** |

### 5.1 Concrete next engineering wedge (Session N+1)

1. **Project real PRs** for polled repos (AI-Manager first) — nodes + review/label fields.  
2. **Attach organic Intent** from labels/title (`do not merge` → FREEZE, `blocked` → BLOCKED, ready+no freeze → SHIP candidate).  
3. **Tag** `source: seed|github_rules|slack_rules` on every Intent/Conflict; pulse already has demo_* fields — digests must honor the same.  
4. **Re-run** `scripts/intent_adequacy_pack.py` → expect `pulse_conflicts_demo_tagged < pulse_conflicts` or honest empty live list.  
5. Show **one live conflict card** on cockpit or documented empty state with reason.

### 5.2 Definition of done (intent bottleneck unlocked — pilot)

Unchanged from plan §7, now evidence-backed:

1. Each mapped engineer: ≥1 organic open intent **or** explicit “no active claim.”  
2. ≥1 non-demo conflict card per week of real work **or** clean empty state.  
3. Follow-through score for claims older than 72h.  
4. Profile re-run passes fact + demo hygiene + intent humility (this doc is the baseline).  
5. Champion can explain a conflict without a standup.

---

## 6. What the experiment falsified / confirmed

| Pre-experiment hypothesis (plan §2) | Result |
|-------------------------------------|--------|
| Enough for work profile | **Confirmed** |
| Not enough for trustworthy intent profile | **Confirmed** (0 organic intents) |
| Conflicts detector works but seed-ish | **Confirmed** (5/5 pulse, 7/7 proxy demo-tagged) |
| Only 2 mapped people serious for digests | **Confirmed** |
| Founder-heavy asymmetry | **Confirmed** (197/242) |
| Events useful for approve/dont_send behavior | **Falsified for this dump** — ops mirror only |

---

## 7. Artifacts

| Path | Role |
|------|------|
| `scripts/intent_adequacy_pack.py` | Reproducible pack exporter |
| `plans/packs/2026-08-06_ten_github_neeljoshi18.json` | Frozen pack used for A+B |
| `plans/2026-08-06_intent-capture-and-adequacy.md` | Design plan (unchanged) |
| `plans/2026-08-06_intent-adequacy-experiment.md` | This writeup |

### Re-run

```bash
python3 scripts/intent_adequacy_pack.py \
  --base https://status.neel.world \
  --tenant ten_github \
  --subject neeljoshi18 \
  --out plans/packs/
```

---

## 8. Closing

The product already **compresses work exhaust into status** for two mapped humans. It does not yet **compress meaning**. The essay’s warning applies directly: if we ship “AI org brain” on top of commit heat + demo SHIP/FREEZE, we will look smart in the demo and die on trust in a real pod.

**Intent adequacy score (honest):** trajectory **4/5**, claims **1/5**, conflicts **1/5** (engine 3/5, data 0/5).  
**Next move:** feed the conflict engine real PRs; stop letting story seeds write “blocked” on people’s digests.
