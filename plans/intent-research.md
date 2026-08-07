# Deep research: how the market captures “intent” — and what that means for AI Manager

**Date:** 2026-08-07  
**Status:** Research memo (no implementation in this file)  
**Product:** AI Manager — permissioned engineering context plane  
**Thesis:** [Intent is the new interface](https://neel.world/intent-is-the-new-interface.html)

No implementation in the original research. This is competitive + philosophical analysis for an **in-house intent engine**.

---

## 1. First: there are *three* different “intent markets”

Most news and vendor lists use “intent” for **B2B buyer intent**. That is **not** your product. Mixing them confuses strategy.

| Market | What “intent” means | Who buys | Examples |
|--------|---------------------|----------|----------|
| **A. GTM / buyer intent** | “This *account* is researching software like yours” | Sales/marketing | Bombora, 6sense, ZoomInfo, G2 Buyer Intent, Demandbase |
| **B. Coding-agent intent** | “What the human meant when they asked the agent to build X” | Developers | SpecStory, OpenSpec, GitHub Spec Kit, Cursor/Claude workflows |
| **C. Organizational / work intent** | “What a person/team is *trying to achieve* at work this week — goals, blockers, ship vs freeze, ownership” | Eng leaders + ICs | Glean personal graph (partial), Jellyfish/Swarmia/LinearB (metrics, not claims), Range (explicit check-ins), Gong/Oliv (sales process intent) |

**AI Manager lives in C**, with a sharp slice of **engineering status + conflict**, and a *adjacent* relationship to B (agents need durable intent). You are **not** competing with Bombora. You are competing for the exec question:

> “What is my eng org *trying* to do, where does it *collide*, and can we kill the standup theater?”

Your essay (“intent is the new interface”) is closer to **C + B** than to **A**.

---

## 2. Map of companies that matter (and those that only *sound* related)

### Tier 0 — Language cousins, wrong product (know them, don’t chase them)

**Buyer-intent platforms** (Bombora, 6sense, ZoomInfo Intent, G2, Cognism, Factors, etc.) capture *purchase research* across the web (co-browsing, review-site visits, content topics). Their “intent engine” is: **aggregate anonymous research signals → account score → sales play**.

**Lesson for you (architecture only):** they win by  
1) **fixed question vocabulary** (“in-market for X?”),  
2) **evidence trails**,  
3) **never claiming mind-reading** — they claim *behavioral likelihood*.  
You should steal the *discipline*, not the data source.

---

### Tier 1 — Closest philosophical cousins (study deeply)

#### **Glean — Enterprise Graph + Personal Graph**

Glean is building a **system of context**: org knowledge graph + **personal graph** of individual work. Public blog (Jun 2025) is unusually candid:

**How they capture “what people are working on”:**

1. **Crawl everything** (docs, Slack, code metadata, calendars, etc.) with **permission-preserving** edges.  
2. Entity extraction → graph triples `(subject, predicate, object)` with timestamps, ACL, provenance.  
3. **Personal graph:** dense activity stream (including reading/passive signals), then **LLM clustering** of atomic actions into tasks → themes → OKRs.  
4. Collaboration edges from comments, shared channels.  
5. Goal: **proactive assistance** — priorities, **conflicts**, organization.  

**What they admit is hard:**

- Not all real work is digital.  
- Inferring *true intent* of a doc edit (draft vs polish) often needs **explicit marks**.  
- Federated search is **not enough** — you need a **common chronological schema** across sources.  
- LLMs fail at multi-hop deterministic questions (“list all AEs in Asia”, “which commits map to OKR X”) — **graphs hold the structure**.  

**Relationship to AI Manager:**

| Glean | You |
|-------|-----|
| Horizontal Work AI / search + agents | Vertical eng **status + conflict plane** |
| Maximize *context coverage* | Maximize *claim fidelity* on work |
| Personal graph of activity clusters | Typed intents (SHIP/BLOCKED/…) + veto loop |
| Risks becoming “everything brain” | Explicit anti-Glean doctrine |

**Steal:** triples + provenance + ACL + “personal graph clusters activity into work units.”  
**Don’t steal:** full-corpus search product; LOC-adjacent rankings; silent 1:1 wiretap.

#### **SpecStory — “Intent is the new source code”**

Almost the same slogan as your essay, but applied to **AI coding sessions**. They capture **prompt/spec history alongside generated code** so the *why* survives. Founded ~2024 to fix what coding tools miss: preserving human intent.

**Capture model:** explicit + ambient **in the IDE** — every agent conversation becomes searchable, durable knowledge.

**Lesson:** Intent without a **durable artifact** dies. Chat is ephemeral; **versioned claims** win. For you, digests + intent nodes are that artifact — not chat logs as product.

#### **OpenSpec / Spec Kit / intent-driven.dev — Spec-driven development**

Pattern: before agents code, humans and AI align on **proposal.md, delta specs, design, tasks** in git. Intent is a **source-of-truth document**, not a vibe.

**Capture model:** **deliberate authoring** of intent before action. High trust, high friction.

**Lesson:** Execs will trust **structured, supersedable claims** more than inferred narratives. Your veto/edit loop is the productized form of “align on intent before status becomes truth.”

#### **Oliv AI — “Intent Graph” for revenue (method gold, domain different)**

Oliv markets an **Intent Graph** for B2B revenue: not generic RAG. They train **100+ small specialized models**, each for a **canonical revenue question** (pains, budget, champion, risk, MEDDIC, …), pass **full conversation history** into cheap SLMs, and continuously update a scorecard. They claim generic LLM+RAG loses speaker/context and hallucinates.

**This is one of the sharpest architectural analogies for you:**

| Oliv | AI Manager intent engine |
|------|---------------------------|
| ~100 fixed revenue questions | Fixed eng **intent types + conflict kinds** |
| Call/email transcripts | GitHub + tickets + channel snippets + digests |
| SLM per question | Rules first → later specialized classifiers |
| Ambient capture of all calls | Ambient capture of work exhaust (not private DMs) |
| CRM as system of record | **Graph as system of record for claims** |

**Steal hard:** “intent = answering a closed set of operational questions with evidence, not open-ended mind reading.”

#### **Gong / Chorus — conversation intelligence**

They capture **every sales conversation**, then extract risk, objections, next steps, forecast signals — intent of the *buyer* and health of the *deal*. Scale of training data is the moat.

**Lesson for eng:** the winning pattern is **always-on capture of a high-signal surface** (calls for sales; for eng: **PRs, CI, tickets, team channels**, bot DMs) + **extract structured risk/claim objects**, not “search the transcript library” as the product.

---

### Tier 2 — Engineering “intelligence” (adjacent; often metrics, not intent)

| Company | What they actually capture | Intent? |
|---------|---------------------------|---------|
| **Jellyfish** | Investment allocation, AI-SDLC observability, Jira+Git | **Trajectory + allocation**, not “what Alice is trying to ship” as a claim |
| **Swarmia** | DORA/SPACE, signals, working agreements, Slack nudges | **Process intent** (agreements), weak on personal purpose |
| **LinearB** | Cycle time, PR flow, AI PR helpers, iteration summaries | **Delivery friction**, some narrative automation |
| **DX (getdx)** | Surveys + metrics (SPACE/Core 4) | **Stated experience**, not work intent graph |
| **Faros, Pluralsight Flow, etc.** | Engineering analytics | Same cluster |

These sell to **VPs of eng** who want boards and throughput. Execs *also* want that — but they still hold standups because metrics **don’t answer “what are we blocked on and who disagrees?”**

**Your wedge vs SEI platforms:**  
They answer *how fast / where time goes*.  
You answer *what people claim they’re doing, where claims collide, and can we stop narrating it in meetings*.

---

### Tier 3 — Explicit status / check-in products

**Range.co** (and Geekbot, Standuply-class tools): people **write** “done / doing / blocked.” Intent is **self-reported**. High accuracy when used; dies when people skip.

**Lesson:** Self-report is gold **when rare and high-stakes**. Your product already does better default: **system drafts from work exhaust; human vetoes**. That’s Range’s accuracy without Range’s daily tax — *if* intent extraction is good enough.

---

### Tier 4 — Design / mechanical “design intent”

**CoLab** and similar: capture design-review decisions so AI agents get engineering design intent. Capture = **review activity as knowledge**, not extra admin.

Same pattern: **collaborative exhaust → reusable intent**, not more forms.

---

### Tier 5 — Platform giants (long-term gravity)

**Microsoft Work IQ / Graph / Copilot:** work graph from email, meetings, chats, docs, calendars — agents with org awareness. Same “system of context” story as Glean, with distribution advantage.

**You cannot out-graph Microsoft or Glean on breadth.** You win on **eng-native claim ontology + conflict + veto-first delivery**.

---

## 3. How the serious players *actually* figure out intent (patterns)

Across domains, the winners converge on a stack. Map each to engineering.

### Pattern 1 — **Closed ontology, not free text as truth**

- Oliv: ~100 revenue questions.  
- Gong: deal stages, risks, competitors, next steps.  
- Buyer intent: topic taxonomies.  
- Your v0: `SHIP | BLOCKED | FIX | EXPLORE | REVIEW | FREEZE | OTHER` + conflict kinds.

**Insight:** Intent becomes product when it’s a **typed object** execs can query: “all FREEZE claims this week,” “open BLOCKED without owner.” Free-text “summaries of activity” is Glean/SEI territory.

### Pattern 2 — **Evidence or it didn’t happen**

Glean attaches provenance and ACL to edges. Gong ties insights to call moments. SpecStory ties intent to session history.

**Insight:** Every intent node needs `evidence[]` (PR id, message permalink, ticket, digest edit). That’s how you sell to security-conscious execs and avoid trust death from your essay.

### Pattern 3 — **Ambient capture of a *chosen* surface**

| Domain | Ambient surface |
|--------|-----------------|
| Sales CI | All customer calls |
| Buyer intent | Web research co-ops / review sites |
| SpecStory | IDE agent chats |
| Glean | Broad enterprise crawl |
| **You** | GitHub (work), tickets, **team Slack channels**, bot DMs — **not** private 1:1 |

**Insight:** Ambient ≠ omniscient. Ambient = **always on for surfaces the org already treats as semi-public work**.

### Pattern 4 — **Infer trajectory; confirm claims**

Glean is honest: inferring intent of a doc edit is hard without explicit marks.  

**Two-layer model (industry-proven):**

1. **Trajectory** (facts): commits, PR state, CI, review stalls — high confidence, low purpose.  
2. **Claims** (intent): labels, titles, “blocked on”, freeze, bot “I’m working on X”, channel decisions — medium/high confidence purpose.  
3. **Follow-through**: claim at t0 vs facts later — **this is where twin becomes real**.

Your experiment already showed: trajectory works; claims are the hole.

### Pattern 5 — **Conflict as the high-value product surface**

Glean explicitly lists **highlighting conflicts** as a personal-graph goal. Sales CI products sell **deal risk** (claim vs reality).  

**Insight:** Execs pay for **collision detection** more than for beautiful profiles. SHIP vs FREEZE, dual owners, merge friction, blocked-without-owner are **board-legible**. Soft preference is privacy poison.

### Pattern 6 — **Specialized models over general chat**

Oliv’s thesis: general LLMs + RAG are the wrong stack for operational intent. Prefer **narrow classifiers** over “ask GPT what Alice wants.”

**Insight for in-house engine:**  
**Rules v0 → labeled gold from digests/vetoes/channel extracts → specialized classifiers.** Do **not** make “one big LLM reads Slack and invents goals” the core. That path is both untrustworthy and outsourced-feeling.

### Pattern 7 — **Human gate on publication**

Range requires human composition. Your digests require Approve / Edit / Don’t send. Gong still leaves action to humans.

**Insight:** Intent that **auto-acts** without a gate is where trust dies (your essay). Intent engine **proposes**; humans **ratify**.

---

## 4. What “mastering intent” means for *your* product specifically

Grounded in AI Manager as built:

**Product already solves the pipeline:**  
GitHub → graph → digests → Slack delivery → veto → Neon.

**Intent is the bottleneck** between “status that writes itself” and “AI organization substrate.”

### What intent is *for* you (definition for execs)

> A **permissioned, evidence-backed claim about purpose** attached to a person and a work object, with confidence, lifecycle (open / superseded / resolved), and conflict detection against other claims and against facts.

Not: mood, productivity score, “who is the best engineer,” full chat archive.

### How you will *look at* intent (surfaces)

| Audience | What they see |
|----------|----------------|
| **IC** | “Your open intents” + digest; edit/veto; bot “I’m blocked on X” |
| **Champion** | Pulse: conflicts, open blockers, dual owners; person profile: claims + follow-through |
| **Exec (later)** | Org heatmap of **claim types and conflicts**, not LOC; “where wills collide” |

### How you will *figure out* intent (in-house engine — no outsourcing the core)

**Layer 0 — Facts (you mostly have)**  
Commits, PRs, CI, reviews, graph edges. Trajectory only.

**Layer 1 — Claim extractors (must be in-house)**  
Deterministic + eventually trained:

| Source | Signal | Intent type bias |
|--------|--------|------------------|
| PR draft / “do not merge” / freeze labels | FREEZE |
| “blocked on”, waiting on review | BLOCKED |
| feat/ship/release language | SHIP |
| fix/hotfix | FIX |
| spike/poc | EXPLORE |
| Ticket status | claim + lifecycle |
| Channel short text (bot present) | claim candidates |
| Bot DM / slash capture | **highest-trust claims** |
| Digest **edits** | gold: human corrected narrative |

**Layer 2 — Conflict resolver (product gold)**  
Rules you already started: ship_vs_freeze, dual_owners, open_blocker, merge_friction, stale_review, ci_blocked. Expand with **organic** PR density.

**Layer 3 — Follow-through**  
Claim → later facts: supported / contradicted / abandoned. This is **digital twin substance** without surveillance rankings.

**Layer 4 — Personal / team intent graph**  
Not Glean’s full activity cluster as product — a **sparse, high-precision claim graph** execs can trust.

**What you refuse to outsource:**

| If outsourced | Failure mode |
|---------------|--------------|
| LLM API as “the intent brain” | No moat; hallucinated goals; trust death; margin tax |
| Buyer-intent vendor APIs | Wrong ontology |
| Glean as dependency | You’re a thin UI on their context |

**What you may use as commodity:**  
transcription APIs, embedding models for *similarity*, cloud LLMs for **prose rewrite of already-structured claims** (your ADR path) — never for inventing work items.

---

## 5. Competitive positioning (one paragraph for sales)

> **Jellyfish/Swarmia/LinearB** tell you how engineering *moves*. **Range** makes people *type* status. **Glean** indexes how the company *knows*. **SpecStory/OpenSpec** preserve intent for *coding agents*. **Gong/Oliv** extract process intent from *revenue conversations*.  
> **AI Manager** extracts **engineering purpose claims and conflicts** from work exhaust + explicit gates, delivers status with veto, and builds an in-house **intent graph** so standups die and agents later act on *ratified* intent — not guesses.

That is a real category gap. Few companies own **typed eng intent + conflict + chat delivery** as one product.

---

## 6. Risks the market teaches (so you don’t copy failures)

1. **Activity ≠ intent** (Glean’s own caveat on doc edits).  
2. **Breadth without ontology** → search product, not status product.  
3. **Metrics platforms** get bought by VPs; ICs hate rankings — you already forbid LOC.  
4. **Full Slack archive** invites competitors (Glean-class) and legal pain; **claim extract** is defensible.  
5. **Eager agents** that act on half-formed intent destroy trust (your essay + every CI failure story).  
6. **Demo seeds as “intelligence”** destroy pilot trust — your experiment proved this; market would too.

---

## 7. Research conclusions (clear idea of the path)

### What “looking at intent” means operationally

You look at a **claim ledger**, not a vibe dashboard:

- Open claims by person and by work item  
- Conflicts (claim–claim and claim–fact)  
- Follow-through scores  
- Human ratifications (approve/edit/don’t send, supersede)

### What “figuring out intent” means technically

1. **In-house extractors** on eng surfaces (rules → gold → narrow models).  
2. **Graph as system of record** for claims (like Oliv’s scorecard, Glean’s triples).  
3. **Conflicts first** for exec value and easier visibility.  
4. **Explicit capture** (bot, slash, labels) for high-confidence.  
5. **Never outsource the claim brain**; optional cloud only for language polish.

### Why this is the deal-breaker for company execs

Execs buy:

- Predictability of delivery  
- Early warning of collisions  
- Less meeting tax  
- A map of *will* (what people are trying to do), not just *motion* (what they typed into git)

Metrics vendors sell motion. Search vendors sell knowledge. **Intent vendors sell alignment.** That’s the huge upside — and why an in-house engine is non-negotiable: **if someone else owns the claims about your customer’s eng org, you are a UI on their moat.**

---

## 8. Suggested further reading

- Glean: personal graph / knowledge graph agentic engine post  
- Oliv: Intent Graph / SLM architecture pages  
- SpecStory: company + “intent is the new source code”  
- OpenSpec / GitHub Spec Kit blogs  
- One SEI comparison (Jellyfish vs Swarmia vs LinearB vs DX)  
- Gong deal-risk methodology as analogy for conflict cards  
- Optional: Microsoft Work IQ, DevRev, Linear project updates  

---

## Bottom line

**How we look at intent:** as a sparse graph of **typed, evidenced, human-gated claims** and their **conflicts**, not as a chat search box or a productivity score.

**How we figure it out:** ambient work exhaust + closed eng ontology + conflict rules + follow-through, trained over time on **our** gold (edits, vetoes, bot statements) — **in-house**, Oliv-style specialization, Glean-style structure, SpecStory-style durability, without becoming Glean or Bombora.

**Why it sells:** execs already pay fortunes for *motion* and *knowledge*; almost no one cleanly owns *engineering will* with veto-first delivery. That’s the category to master — and the reason the product you have now is real, but **intent is the multiplier**.
