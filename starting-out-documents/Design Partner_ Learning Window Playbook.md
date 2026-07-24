# Design Partner — Learning Window Playbook (10–14 days)

**Audience:** Operator running a multi-member beta install  
**Product name for customers:** **Learning window** (not “we stopped watching”)  
**Sequence lock (ADR-016):** multi-person digests + shadow gold **before** local model training

---

## 0. Preconditions (beta gate)

- [ ] Staging/product stack healthy (`/app/` Connections green; last ingest age fresh)  
- [ ] GitHub webhooks → V1 → bridge → V2  
- [ ] Slack egress vault wired (no tokens in twin env)  
- [ ] **≥2 humans** on Team map (`/app/` → Team)  
- [ ] `NOTIFY_INTERVAL_SECS` / `STATUS_WINDOW_SECS` set (defaults 30m / 1h)  
- [ ] High auto-publish **off**; shadow or learning-mode copy on digests  

---

## Day 0 — Kickoff

1. **Connectors:** GitHub App + Slack bot (vault).  
2. **Map team:** For each human: subject / GitHub login aliases → Slack user ID.  
3. **Explain:** Ingest is continuous; Slack is batched; veto is first-class.  
4. **Show:** Today conflicts card + My status draft actions.  
5. **Baseline metrics:** Record `/metrics` DMs, veto rate, empty windows = 0.  

**Do not** promise full custom LLM of the company. Structure-first digests are the product.

---

## Days 1–13 — Shadow operations

| Cadence | Action |
|---------|--------|
| Continuous | Ingest GitHub (later tickets); graph + intent rules update |
| Every window | Compile ledgers; DM only if non-empty + notify interval |
| Daily (champ) | Glance veto/edit; note wrong claims (evidence missing?) |
| 2–3× / week | Check **Team blockers & conflicts** on Today |
| Weekly | Optional cancel **one** standup if digests cover status |

### Operator checks

```text
GET /metrics
  twin_dms_sent_total
  twin_veto_rate
  twin_empty_windows_total
  twin_conflict_hits_total

GET /v3/tenants/{tenant}/team      → multi_person_ready == true
GET /v3/tenants/{tenant}/pulse     → conflicts + intents sample
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
| Multi-person digests | Team map ≥2; DMs > 0 for >1 subject | Both humans got ≥1 digest |
| Veto rate | `vetoes / (vetoes + publishes)` | Not 100% (broken) or 0% with angry users |
| Empty windows | `twin_empty_windows_total` | Expected quiet periods OK |
| Conflict surface | Pulse conflicts count | ≥1 useful card during window |
| Standups canceled | Human report | ≥1 status meeting skipped |
| Gold pairs | Edited/approved draft text | Keep for M7; do not train yet if sparse |

### Export checklist (for later M7 — optional)

- Approved / edited draft text + structured ledger JSON  
- Intent labels (type + about node + owner)  
- Veto reasons if captured  

**Do not** dump raw git blobs into a training set (ADR-006).

---

## Decision after window

| Path | When |
|------|------|
| **A. Stay rules (+ optional cloud prose later)** | Mid-market, no GPU, digests already good |
| **B. Enable Model Router → customer-prem SLM** | After gold pairs exist; privacy/cost pitch |
| **C. Extend learning window** | Veto rate high; mapping incomplete |

---

## Incident / rollback

| Issue | Action |
|-------|--------|
| Slack spam | Raise `NOTIFY_INTERVAL_SECS`; confirm bridge never DMs |
| Wrong person mapped | Team UI fix aliases; bridge refreshes map ≤60s |
| Secrets leak risk | Rotate vault only; never put tokens in twin env |
| Partner wants search | Re-state anti-Glean; offer graph neighborhoods only |

---

## Related docs

- `Product Roadmap_ Intent Capture to Digital Twins.md` § M6 / M6.5  
- `plans/2026-07-24_onprem-model-and-agents.md`  
- `Architecture Decision Log` ADR-012–016  
- `Design Partner_ One-Pager.md`  
