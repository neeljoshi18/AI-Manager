# Interaction Log — Product & Architecture Decisions

**Purpose:** Durable log of founder ↔ build sessions so future chats don’t rely on compressed context.  
**Repo path:** `starting-out-documents/Interaction Log_ Product Decisions.md`

---

## 2026-07-22 — Session: V3 implement → Sew & Show → GitHub live → batch notify → roadmap

### Context

- Monorepo AI Manager; private GitHub `neeljoshi18/AI-Manager`.  
- V1/V2/security already largely built; V3 was spec-only.

### Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Implement V3 per TAS (twins, ledger, veto, egress Slack) | Product layer where pitch becomes real |
| 2 | **Do not** build Buzz/Centaur clones | ADR-011 |
| 3 | Demo console on `:18083/demo/` | Founder visibility; lead-friendly |
| 4 | Real Slack via egress vault only | ADR-012; no bot token in twin env |
| 5 | Full stack sew V1+V2+V3 before “agent OS” | Empty graph → empty product |
| 6 | Live GitHub via ngrok + webhook secret | Prove real PR path |
| 7 | **Ingest continuous, notify batched** | User got ~15 DMs per PR activity; unacceptable |
| 8 | Defaults: 1h status window, 30m notify interval | Configurable env knobs |
| 9 | Bridge projects graph only; twin-api owns DMs | Separation of ingest vs delivery |
| 10 | Private 1:1 DM wiretap is **out**; bot-mediated / opt-in only | Slack platform + ethics |
| 11 | Next: productize onboarding, ticketing, Slack inbound, then intent/conflict agents | Full app goal without scope collapse |
| 12 | Plan captured in `Product Roadmap_ Intent Capture to Digital Twins.md` | Single backlog spine |

### Artifacts produced

- `vertical-3/` full crates + migrations + demo-static  
- `scripts/platform_sew.sh`, `scripts/github_live_bridge.py`  
- `Human Demo Script.md`, `GitHub Webhook Setup_ Local.md`, `Plan_ Sew and Show M4.md`  
- Git push: commit `1cb28c1` (and subsequent if any)  

### Open questions (for later sessions)

1. GitHub App vs long-lived ngrok for partners?  
2. First ticketing connector: Jira vs Linear?  
3. Cloud host for M5 (Fly / Render / bare VM)?  
4. When to open ADR for **V4 Intent & Conflicts**?  

---

## Template for future entries

```markdown
## YYYY-MM-DD — Session title

### Context
### Decisions (table)
### Artifacts
### Open questions
```

---

*Append only; never rewrite history of past decisions—supersede with a new entry.*
