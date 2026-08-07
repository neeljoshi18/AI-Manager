# Intent Engine design (in-house)

**Date:** 2026-08-07  
**Research:** `plans/intent-research.md`  
**Doctrine:** Chat = delivery · GitHub = work · no LOC rankings · no silent 1:1 wiretap · human gate on publish

## Definition

An **intent** is a permissioned, evidence-backed **claim about purpose** attached to a person and (optionally) a work object — not activity volume, not chat archive, not mood.

## Seven principles (product law)

| # | Principle | Implementation |
|---|-----------|----------------|
| 1 | **Closed ontology** | `SHIP \| BLOCKED \| FIX \| EXPLORE \| REVIEW \| FREEZE \| OTHER` only |
| 2 | **Evidence or it didn’t happen** | Every claim has `evidence[]` + `source` |
| 3 | **Chosen ambient surfaces** | GitHub · tickets · team channels (bot present) · bot DM — not private 1:1 |
| 4 | **Trajectory ≠ claim** | Facts layer separate; claims explicit or classified with confidence |
| 5 | **Conflicts first** | Claim–claim and claim–fact collisions are the exec product |
| 6 | **Rules before LLM** | In-house classifiers; no outsourced “mind read” brain |
| 7 | **Human gate** | Digests Approve/Edit/Don’t send; claims can be superseded; engine proposes |

## Layers

```
L0 Facts     → graph commits/PRs/CI (trajectory)
L1 Claims    → extractors (rules) + explicit capture
L2 Conflicts → detect collisions (V2 + pulse)
L3 Follow-through → claim vs later facts
L4 Surfaces  → ledger API · profile · cockpit · digests
```

## APIs (V3)

| Method | Path | Role |
|--------|------|------|
| GET | `/v3/tenants/{t}/intent/engine` | Principles + health + counts |
| GET | `/v3/tenants/{t}/intent/ledger` | Unified claim ledger (graph + slack + explicit) |
| POST | `/v3/tenants/{t}/intent/claims` | Explicit capture (bot/slash/champion) |
| POST | `/v3/tenants/{t}/intent/claims/{id}/supersede` | Human supersede |
| GET | `/v3/tenants/{t}/people/{s}/profile` | Person view (existing) |
| GET | `/v3/tenants/{t}/people/{s}/follow_through` | Follow-through (existing) |
| GET | `/v3/tenants/{t}/pulse` | Conflicts first surface |

## Storage

| Store | What |
|-------|------|
| V2 graph Intent nodes | Organic + seed claims (system of record for work-attached) |
| tenant_kv `slack_intent_claims` | Channel/DM extracts ≤280 |
| tenant_kv `intent_explicit_claims` | Human/bot explicit ledger ring |
| twin digests | Narrative + gold (edits/vetoes) |

## Non-goals

- Buyer intent (Bombora-class)
- Full Slack/doc search (Glean-class)
- LOC / productivity rankings
- Auto-acting agents on unratified claims
