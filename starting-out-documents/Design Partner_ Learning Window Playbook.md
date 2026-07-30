# Design Partner — Learning Window Playbook (10–14 days)

**Audience:** Operator running a multi-member beta install  
**Product name for customers:** **Learning window** (not “we stopped watching”)  
**Sequence lock (ADR-016):** multi-person digests + shadow gold **before** local model training  
**Install first:** `Design Partner_ Install Runbook.md`

---

## 0. Preconditions (beta gate)

- [ ] Staging/product stack healthy (`/app/` Connections green; graph filled or clearly recovering)  
- [ ] GitHub webhooks → V1 → bridge → V2  
- [ ] Slack egress vault wired (no tokens in twin env)  
- [ ] **≥2 humans** on Team map (`/app/` → Team); `multi_person_ready == true`  
- [ ] Notify Policy v1 live (`/metrics` → `notify_policy: v1_change_only_daily_cap`)  
- [ ] High auto-publish **off**; shadow or learning-mode copy on digests  

---

## Day 0 — Kickoff

1. **Connectors:** GitHub App + Slack bot (vault).  
2. **Map team:** For each human: subject / GitHub login aliases → Slack user ID.  
3. **Explain:** Ingest is continuous; Slack is rare (change-only + max 1 status DM / person / UTC day).  
4. **Show:** Today conflicts card + My status **Approve / Edit / Don't send**.  
5. **Baseline metrics:** Record `/metrics` DMs sent, suppressed, Don't-send rate, empty windows.  

**Do not** promise full custom LLM of the company. Structure-first digests are the product.

---

## Days 1–13 — Shadow operations

| Cadence | Action |
|---------|--------|
| Continuous | Ingest GitHub (later tickets); graph + intent rules update |
| Every window | Compile ledgers; DM only if non-empty **and** Notify Policy allows |
| Daily (champ) | Glance Approve/Edit/Don't send; note wrong claims (evidence missing?) |
| 2–3× / week | Check **Team blockers & conflicts** on Today |
| Weekly | Optional cancel **one** standup if digests cover status |

### Operator checks

```text
GET /metrics
  twin_dms_sent_total
  twin_dms_suppressed_total   # should dominate if anti-spam works
  twin_veto_rate              # Don't-send rate (API name)
  twin_empty_windows_total
  twin_conflict_hits_total
  notify_policy

GET /v3/demo/status           → graph_status ok|empty|v2_down
GET /v3/tenants/{tenant}/team → multi_person_ready == true
GET /v3/tenants/{tenant}/pulse → conflicts + intents sample
```

### Intent types (rules v0)

SHIP · BLOCKED · FIX · EXPLORE · REVIEW · FREEZE · OTHER  

Source: PR/issue titles, labels, body keywords. **No LLM invent.**

### Conflict kinds (v0)

- Dual owners / competing claims on same work  
- SHIP vs FREEZE  
- Mutual or ship-blocking BLOCKS edges  
- Open BLOCKED intents  

Human resolve only — no autonomous code changes.

### Privacy red lines

- No silent private 1:1 DM wiretap  
- Bot-mediated or opt-in capture only if you add intent bot later  
- No LOC rankings in any surface  

---

## Day 14 — Closeout scorecard

| Metric | How | Healthy signal (guideline) |
|--------|-----|----------------------------|
| Multi-person digests | Team map ≥2; DMs &gt; 0 for &gt;1 subject | Both humans got ≥1 digest |
| Don't-send rate | `vetoes / (vetoes + publishes)` | Not 100% (broken) or 0% with angry users |
| Suppressions | `twin_dms_suppressed_total` | Rising while work is steady = anti-spam OK |
| Empty windows | `twin_empty_windows_total` | Expected quiet periods OK; empty never DMs |
| Conflict surface | Pulse conflicts count | ≥1 useful card during window if conflicts exist |
| Standups canceled | Human report | ≥1 status meeting skipped |
| Gold pairs | Edited/approved draft text | Keep for M7; do not train yet if sparse |

### Export checklist (for later M7 — optional)

- Approved / edited draft text + structured ledger JSON  
- Intent labels (type + about node + owner)  
- Don't-send reasons if captured  

**Do not** dump raw git blobs into a training set (ADR-006).

---

## Decision after window

| Path | When |
|------|------|
| **A. Stay rules (+ optional cloud prose later)** | Mid-market, no GPU, digests already good |
| **B. Enable Model Router → customer-prem SLM** | After gold pairs exist; privacy/cost pitch |
| **C. Extend learning window** | Don't-send rate high; mapping incomplete |

---

## Incident / rollback

| Issue | Action |
|-------|--------|
| Slack spam | Confirm Notify Policy v1; raise intervals only if needed; bridge never DMs |
| Wrong person mapped | Team UI fix aliases; bridge refreshes map ≤60s |
| Secrets leak risk | Rotate vault only; never put tokens in twin env |
| Graph empty 0/0 | Wait &lt;2 min recovery; autoheal V2; check Connections graph_status |
| Partner wants search | Re-state anti-Glean; offer graph neighborhoods only |

---

## Related docs

- `Design Partner_ Install Runbook.md`  
- `Design Partner_ One-Pager.md`  
- `Product Roadmap_ Intent Capture to Digital Twins.md` § M6 / M6.5  
- `plans/2026-07-24_onprem-model-and-agents.md`  
- `plans/2026-07-27_confidence-airtight-pilot.md`  
- `Architecture Decision Log` ADR-012–016  
