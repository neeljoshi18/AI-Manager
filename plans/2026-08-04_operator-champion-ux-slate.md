# Operator / Champion UX slate (2026-08-04)

## North star (from sales vision)

1. **Identity** — company SSO (later); seats champion vs member  
2. **Connect** — Slack *or* Teams + GitHub App  
3. **Map pod** — manual (live) → auto-suggest → self-invite  
4. **Shadow** — graph fills; rare digests  
5. **Champion cockpit** — pulse, digests, conflicts, heat, graph jump, tomorrow focus  
6. **IC delivery** — Approve / Edit / Don't send on chat  
7. **Adapters** — Teams after Slack; other chat later  

## Build order (execute in this sequence)

| # | Item | Status |
|---|------|--------|
| 0 | Sales Call Documents + leave-behind one-pager | **Done** |
| 1 | **Champion cockpit UI** (package live APIs) | **Done** |
| 2 | Team mapping: bulk paste + clearer copy (Path A) | **Done** |
| 3 | Connect Slack / GitHub real install | **Done this session** (status API, Slack callback→vault, GH install URL + webhook copy, UI) |
| 4 | Delivery abstraction + **Teams bot** | **Next session** |
| 5 | Roles champion/member | **Next session** |
| 6 | Google/SSO join | **Next session+** |
| 7 | Tomorrow focus board (persist assignments) | Scaffold live; **persist next session** |
| 8 | Other chat adapters | After Teams |

**Session boundary:** Items **4–8** ship in the **next session** (see `starting-out-documents/Session Handoff_ Context Transfer 2026-08-04c.md`).

## Cockpit panels (v1 — this batch)

| Panel | Data source |
|-------|-------------|
| Readiness | `GET …/pilot_readiness` |
| Pod roster | `GET …/team` |
| Conflicts / intents | `GET …/pulse` |
| Heat | `GET …/insights/dev` |
| Graph jump | Navigate Graph + `…/graph` stats |
| Actions | Compile digests, enrich story, open status |
| Tomorrow focus | Derived from conflicts + intents + open digests (client-side v1) |

## Doctrine

No individual LOC rankings · no private 1:1 wiretap · chat = delivery · GitHub = work.
