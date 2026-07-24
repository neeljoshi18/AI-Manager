# Plan: Shadow ingest → agents → on-prem model inference (vision lock)

**Date:** 2026-07-24  
**Mode:** Plan (docs + sequencing; implementation after approval)  
**Ground truth to update after approval:** Product Roadmap, ADR log (+ new ADR-016), Interaction Log, Session Handoff, `plans/YYYY-MM-DD_onprem-model-and-agents.md`

---

## 1. Vision you described (refined into product language)

### What stays always-on

| Layer | Behavior forever |
|-------|------------------|
| **Ingest** | Continuous (GitHub, later Jira/Linear/Slack channels). Never “stop learning exhaust.” ADR-014. |
| **Graph** | V2 accumulates entities, edges, blockers, later intents. |
| **Twins / status** | Compile windows + batched notify + human veto. |

### What changes after the “ingestion / shadow period” (≈10–14 days)

Today’s roadmap already has **shadow mode** (compile digests without trusting auto-publish). Your new piece:

| Period | Inference / “brain” | Cost & privacy |
|--------|---------------------|----------------|
| **Shadow / design-partner start (days 0–14)** | Mostly **deterministic** rules + graph; optional **paid LLM via egress** only for prose rewrite from structured fields | Higher $; secrets stay egress (ADR-012); good for bootstrap quality |
| **After shadow, when customer is “live”** | Prefer **customer-prem local SLM/LLM inference** (sandbox on *their* GPU/CPU box or their VPC). Agents/workers call a **Model Router** → local first | Lower unit inference cost; data plane stays in customer boundary |
| **Our cloud multi-tenant path (later)** | May still use managed models behind egress for small tenants who refuse on-prem GPU | Optional SKU, not the enterprise default story |

**Important honesty (keep thesis intact):**

- We do **not** host proprietary source code or build a Glean-class vector corpus (ADR-006).  
- “Train locally” means: **adapt / distill / fine-tune a small open model (or LoRA) on structured company signals** (ledgers, intent labels, approved status text, graph-derived features)—not “upload the monorepo into OpenAI forever,” and not “we keep a copy of their Drive.”  
- Agents still **watch and draft**; humans **veto** (ADR-011). Local model does not get god-mode deploy keys without egress allowlists.

### Why this is on-thesis

1. **Cost** — Status digests + conflict prose + intent typing at mid-market volume should not be per-token SaaS tax forever.  
2. **Privacy** — Customer brain stays in their sandbox; we sell the **context plane + agents**, not a third-party memory of their org.  
3. **Differentiation** — Competitors sell cloud copilot seats; we sell **meeting elimination + graph truth** with an optional **on-prem inference SKU** after trust is earned in shadow.

---

## 2. Architecture (target)

```
                    continuous
Sources ──► V1 ──► V2 graph ──► workers / agents
                                  │
                                  ▼
                         Model Router (new)
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
              Local SLM     Cloud LLM      Rules only
              (customer     (egress,       (default v0)
               prem /        optional)
               VPC)
                                  │
                                  ▼
                         V3 ledgers / Slack
                         (veto-first, batched)
```

| Component | Role |
|-----------|------|
| **Model Router** | Single interface: `rewrite_status`, `type_intent`, `draft_conflict_card`. Backend = `rules` \| `cloud` \| `local`. |
| **Shadow corpus** | Not raw code: exportable **training pairs** from graph + ledgers + human edits/vetoes (approved text is gold). |
| **Local runtime** | Customer-owned: Ollama / vLLM / llama.cpp-class serving; we ship compose recipe + weights channel (open base + customer adapter). |
| **Training job** | Offline (after ≥N days + ≥M approved digests): distill/LoRA on customer box or one-shot job they run. We do not require day-one full fine-tune. |
| **Egress** | Still used for Slack/GitHub **tools**; model weights stay local; no bot tokens in twin env (ADR-012). |

### What “train” means in phases (realistic)

| Stage | Method | Needs |
|-------|--------|-------|
| **v0** | No train — rules + templates | Shipping now |
| **v1** | Optional cloud rewrite of structured ledger → prose | Egress + paid key only during shadow if wanted |
| **v2** | Distill: (structured ledger JSON → approved draft text) pairs → small LoRA / prompt-cache local model | 10–14d+ shadow, human edits/vetoes, GPU or strong CPU |
| **v3** | Intent classifier local (SHIP/BLOCKED/…) from labels | Intent v0 rules first, then supervised labels |
| **Later** | Deeper personalization per team | Only after multi-tenant control plane |

Avoid promising “custom GPT of the whole company” without structure-first graph—that reopens Glean economics.

---

## 3. Sequencing: agents first or local model first?

### Clear recommendation

```
M5 staging solid (mostly done)
   → M6 multi-member beta path (connectors + thin agents + conflicts)
        → Design partners on shadow digests (10–14d)
             → Collect training pairs + veto metrics
                  → M7+ on-prem Model Router + local SLM SKU
```

| Priority | Why |
|----------|-----|
| **1. Multi-member product path (agents/monitors + intent/conflict v0)** | This is what makes beta *useful* for a real team and produces the data/behavior you need for local models. Without multi-person graph + digests + conflicts, “local LLM” has nothing trustworthy to say. |
| **2. Design partner shadow (10–14d)** | Ingest continuous; digests in shadow; measure veto rate; kill one standup. |
| **3. Local model inference SKU** | Cost/privacy moat **after** the product works and after you have approved text/intent labels. Training without agents + multi-member exhaust is premature optimization. |

**Do not** implement full local training before:

- Staging golden path stable (HTTPS + GitHub + Slack + bridge + ≤1 DM / window) — **largely true**.  
- At least **one multi-person team** on digests (user map >1 human).  
- Intent/conflict **v0 rules** (no LLM required).  
- Thin agent monitors (graph delta, conflict card, ingest health).

Local model is **Phase after first beta insight**, not the next coding week.

---

## 4. Milestone map (updated spine)

| Phase | Goal | “Done” for beta outreach |
|-------|------|---------------------------|
| **M5 close** | Public staging product; always-on bridge; multi-user Slack map | Stranger can open `status.neel.world`, connect tools, get digests |
| **M6 Design partner** | 2nd connector (Linear/Jira), Slack channel metadata, **intent v0**, **conflict v0**, **2–3 monitor workers**, multi-person twins | You can recruit 1–3 eng teams for 2-week shadow |
| **M6.5 Shadow program** | Fixed playbook: 10–14d shadow, metrics dashboard (DMs, veto %, standups canceled) | Design partner finishes with a scorecard |
| **M7 On-prem inference** | Model Router + local serve recipe + export training pairs from shadow | Customer can turn off cloud LLM and still get digests |
| **M7/M8** | Self-serve multi-tenant; richer agents (negotiator still gated) | Scale beyond founder-operated installs |

### Thin agents to build in M6 (earn their place)

| Agent | Input | Output | Gate |
|-------|--------|--------|------|
| Ingest health | Webhook failures, bus lag | Ops alert | Ops |
| Graph delta → intent candidates | New edges/nodes | Intent nodes (rules) | Rules first |
| Status compiler | Window + graph | Ledger + optional DM | Batch + veto (exists) |
| Conflict detector | Competing intents / dual BLOCKS | Conflict card in UI/Slack | Human resolve |

Still **not** Centaur multiplayer OS / Buzz workspace.

---

## 5. Shadow period — operational product definition

Codify what you already pitch:

| Day | Product behavior |
|-----|------------------|
| **0** | Connect Slack + GitHub (+ ticket system when ready); map humans → Slack |
| **0–14** | **Ingest continuous**; compile ledgers; digests in **shadow** or private DM with explicit “learning mode” copy; high auto-publish **off** |
| **During** | Humans edit/veto → those become **training gold**; intent rules label SHIP/BLOCKED/… |
| **Day 14+** | Offer: (A) stay cloud/rules, (B) enable **local Model Router** with open base + optional adapter trained on their approved pairs |
| **Forever** | Ingest does **not** stop; only inference backend and auto-publish policy may change |

Name in product UI: **“Learning window”** (not “we stopped watching”).

---

## 6. Path to beta teams (when you can reach out)

You can start **soft** outreach when M6 checklist hits these:

| Gate | Why |
|------|-----|
| Multi-person Slack map works (2+ humans, not only founder) | Team product, not solo demo |
| Real PRs → graph → scheduled digest without manual project | Trust |
| Veto/publish from Slack or `/app` | Core loop |
| Conflict or “team blockers” surface v0 | Differentiator vs Geekbot forms |
| One-page design-partner agreement + shadow playbook | Professional beta |

**Local model is not a gate for first beta.** First beta *feeds* the local model story for enterprise later.

Beta ask: *“2 weeks of shadow digests; you cancel 1 standup if veto rate &lt; X; we never silent-read private 1:1s.”*

---

## 7. Document updates (implementation of this plan)

After you approve this plan, **write** (not only talk):

| Document | Change |
|----------|--------|
| `plans/2026-07-24_onprem-model-and-agents.md` | This plan as dated snapshot |
| `starting-out-documents/Product Roadmap_ Intent Capture to Digital Twins.md` | New § on Learning window + Model Router + M6.5/M7 on-prem; reorder “after staging”; explicit “ingest continuous, inference local after shadow” |
| `Architecture Decision Log` | **ADR-016 — Customer-prem model inference after shadow** (options: always cloud / always local day-one / hybrid router; choice hybrid; runner-up cloud forever; revisit triggers) |
| `Interaction Log_ Product Decisions.md` | Append 2026-07-24 decision entry |
| `Session Handoff_ Context Transfer …` | Milestone table + next tasks; note staging live + bridge; local model post-M6 |
| `plans/README.md` | Index new plan |
| Optional | Short `starting-out-documents/On-Prem Model SKU_ Vision.md` if roadmap § gets long |

**No code** in that doc-update slice unless you also want a stub `ModelBackend` trait (can be a follow-on PR).

---

## 8. Open technical choices (defaults if you don’t care)

| Question | Default |
|----------|---------|
| Local serve stack | **vLLM or Ollama** recipe; start with Ollama for mid-market single-box |
| Base model class | Small open instruct model (7B-class) for rewrite; rules for intent until labeled data exists |
| Training | Distillation/LoRA on **approved digests + intent labels**, not raw git blobs |
| Who runs training job | Customer runs on their box (or we run one-shot with their export)—weights leave only if they choose cloud SKU |
| Cloud during shadow | Optional; product works rules-only |

---

## 9. Risks if we get the order wrong

| Risk | Mitigation |
|------|------------|
| Train local model before multi-member product | Sequence: M6 agents/team → shadow → local model |
| Local model invents work items | Structure-first: model only rewrites **from ledger fields**; same as V3 today |
| On-prem = full Centaur clone | ADR-011 still holds; local is **inference**, not agent OS |
| Privacy story collapses if we host training data | Customer-prem export; we don’t keep their gold pairs unless contracted |
| Cost of GPU for every SMB | Rules-only + cloud optional for small; on-prem for privacy-sensitive / high volume |

---

## 10. Success criteria (vision-level)

- [ ] Pitch says: continuous ingest; **learning window**; then **local inference SKU** without lying about “full company GPT.”  
- [ ] Roadmap + ADR-016 committed as ground truth.  
- [ ] Beta path clear: **M6 multi-member + thin agents before local train.**  
- [ ] First design partner can finish 14d shadow with metrics.  
- [ ] Model Router design allows swap rules/cloud/local without rewriting V3.

---

## 11. Immediate next actions after approval

1. Materialize `plans/2026-07-24_onprem-model-and-agents.md` + update roadmap/ADR/handoff/interaction log/plans index.  
2. **Do not** start training code yet.  
3. **Next engineering focus for beta:** multi-person twin maps, intent v0 rules, conflict v0, Slack channel metadata or Linear/Jira one connector, monitor workers.  
4. Parallel product: design-partner one-pager + shadow playbook (can be markdown in `starting-out-documents/`).  
5. Only after one partner is in shadow: spike Model Router + Ollama side-car on a box.

---

## 12. Answer in one paragraph (for you)

**Waiting half an hour is notify policy; local models are a later cost/privacy SKU.** Right now the product wins by continuous ingest + graph + batched veto digests for a **team**. Implement **thin agents + multi-member surfaces next** so you can beta-test with real teams; use the 10–14 day shadow to collect **structured gold** (ledgers, edits, intents). **After** that, train/adapt a small model that runs **on the customer’s server** and have all agents call a **Model Router** (local first, cloud optional via egress)—not the other way around.

---

*Plan mode: no ground-truth files edited until you approve. After approval: doc-only PR first, then M6 engineering.*
