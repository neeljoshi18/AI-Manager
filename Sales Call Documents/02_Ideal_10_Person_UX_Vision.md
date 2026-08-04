# Ideal 10-Person UX Vision (Polished)

**Status:** North-star product experience for sales storytelling  
**Related:** 01 = what you can demo today · 03 = auth/chat · 04 = mapping paths

---

## 1. Vision in one screen

A 10-person eng team:

1. Joins via company identity (optional Google/SSO)  
2. Champion connects **chat** (Slack **or** Microsoft Teams) + **GitHub App**  
3. Employees are **mapped** into the eng pod (not the whole company)  
4. Graph builds quietly (**shadow** — rare or no spam)  
5. Champion runs a **cockpit**: map, conflicts, pulse, heat, digests  
6. ICs get private digests on their chat with **Approve / Edit / Don't send**  
7. Optional **tomorrow focus** from open work + blockers (human-gated)

---

## 2. Phases (end-to-end journey)

### Phase 0 — Join (~5 min / person)

1. Champion sends invite → `app.<pilot-or-customer-host>/join`  
2. Sign in with **Google Workspace / SSO** (identity + tenant seat)  
3. Role: **champion** or **member**  
4. Optional: link chat identity if not already via Connect  

*Today:* white-glove map; no full SSO yet.

### Phase 1 — Connect (champion, 15–30 min)

1. **Connect chat** — pick **Slack** *or* **Teams** (one primary delivery plane per tenant)  
2. **Connect GitHub** — install **GitHub App** once on org/repos  
3. **Map the pod** (doc 04) — auto-suggest + confirm, or manual  
4. Continuous ingest → graph  

**Quiet truth:** “Nothing happens” for a bit = **shadow learning**, not a bug.

### Phase 2 — Quiet learning (days 1–3)

- Graph fills with people, PRs, commits, intents  
- Digests compile; Notify Policy keeps Slack/Teams rare  
- Champion watches cockpit; ICs may see little yet  

### Phase 3 — Champion cockpit (the wow)

| Panel | What they see |
|-------|----------------|
| **Team map** | All mapped people; click → neighborhood on graph |
| **Conflicts** | Work conflicts: dual owners, SHIP vs FREEZE, BLOCKED |
| **Team pulse** | Who has open work / empty window / blockers today |
| **Heat** | When the **team** is most active; optional per-person heat **without** a ranking table |
| **Digests board** | Last draft per person → open Approve / Edit / Don't send |
| **Tomorrow focus (vision)** | Suggested focuses from open PRs + blockers; champion assigns notes — not agent auto-tasking |

### Phase 4 — IC day-to-day

- Private digest on **Slack DM** or **Teams chat** when the story **changes**  
- Approve / Edit / Don't send  
- Optional web “My status” for the same draft  

### Phase 5 — Outcome

- Cancel standup if digests trusted  
- Grow map from 2 → 10  
- Scorecard: suppressions, Don't-send rate, empty windows, standups canceled  

---

## 3. Polish rules (what we never sell)

| Never | Instead |
|-------|---------|
| Individual LOC / “efficiency score” leaderboards | Team heat + work context + conflicts |
| “Conflicts with everybody” as personality | Conflicts on **shared work items** |
| Auto-assign tasks by AI agent | Human-gated **tomorrow focus** |
| Google login = we own eng data | Google = **who you are**; GitHub = **what you ship**; chat = **how we deliver** |
| Whole-company chat index | Only **mapped** eng pod digests |

---

## 4. Live today vs build next

| Capability | Demoable now | Vision |
|------------|--------------|--------|
| Slack digests + Approve loop | Yes | Same |
| Graph people / PR / intents | Yes | Champion packaging |
| Dual digests multi-person | Yes (pilot) | Self-serve 10-seat |
| Dev insights heat | Yes (dogfood) | Team panel in cockpit |
| Microsoft Teams digests | No | Delivery adapter + Adaptive Cards |
| Connect Slack / GitHub buttons | Partial / manual | Real install UX |
| Connect Teams | No | After Slack path solid |
| Google/SSO join | No | With multi-tenant packaging |
| Employee self-invite map | No | Mapping path C |
| Manual map | Yes | Forever fallback |
| Tomorrow focus board | No | Later scoped feature |

---

## 5. Sales language (ideal)

> “Your eng pod signs in with company identity. You connect Slack **or** Teams, install the GitHub App once, and pick who is on the map. We build a permissioned graph of work and intents while staying quiet. Then each engineer gets a rare private status draft they control—and you get a cockpit so the standup can die.”

**If they ask “is that live?”**  
Point to **01** for the pilot path; this doc is the **product destination**.

---

## 6. Product slate (critical only)

1. Champion cockpit packaging (existing APIs)  
2. Connect Slack + Connect GitHub (finish)  
3. Delivery abstraction + **Teams bot** (same actions)  
4. Mapping UX: bulk + auto-suggest  
5. Google/SSO + roles  
6. Tomorrow focus board  
7. Other chat adapters only after Slack+Teams proven  

**Out of scope:** Linear day-one, model training, productivity rankings, silent 1:1 wiretap.
