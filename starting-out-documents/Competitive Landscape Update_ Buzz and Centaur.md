# Competitive Landscape Update: Buzz (Block) & Centaur (Paradigm)

**Date:** 2026-07-22  
**Audience:** Product + engineering  
**Related:** Glean strip-to-win thesis; Vertical 1–2 architecture; Architecture Decision Log  

**Primary sources:**  
- https://engineering.block.xyz/blog/buzz  
- https://buzz.xyz/ · https://github.com/block/buzz  
- https://www.paradigm.xyz/writing/open-sourcing-centaur-multiplayer-self-hosted-secure-agents  
- https://centaur.run/ · https://github.com/paradigmxyz/centaur  
- Block “From Hierarchy to Intelligence” / industry coverage of AI-native org redesign  
- Adjacent: Goose/ACP; credential brokering industry pattern (Infisical, Envoy injectors, Cloudflare/Vercel sandbox egress)

---

## 1. Executive summary

Two open-source platforms landed in the same broad “AI at work” space as AI Manager, but on **different axes**:

| Player | One-line job |
|--------|----------------|
| **Buzz (Block / Jack Dorsey)** | Self-hostable **workspace** where people + agents share signed identity, chat, and Git at agent scale |
| **Centaur (Paradigm / Tempo)** | Self-hostable **secure multiplayer agent runtime** (Slack-native) with sandboxes and **secret injection at the network edge** |
| **AI Manager (us)** | **Strip-to-win engineering context plane**: metadata telemetry + ACL-safe temporal graph that kills status meetings |

**They do not obsolete us.** They compete for budget and narrative (“AI manager / AI coworker”) if we describe ourselves as a chat bot.  
**They do create pressure** on security (secrets) and on whether we eventually expose our graph as **context infrastructure for any agent workspace**.

**Steal, don’t clone:** Centaur’s **egress credential injection** pattern. Optionally Buzz’s **agent identity / delegation** ideas later.  
**Do not build:** a Nostr Slack replacement or a full K8s multiplayer coding agent kernel as our core product.

---

## 2. Buzz — deep brief

### 2.1 What it is

Buzz is Block’s open-source, self-hostable **hive workspace**: humans and AI agents work in the same rooms. It is built on **Nostr** (signed messages, portable keypair identities). The server holds channels, search, automation, and **Git hosting**. Agents (Claude Code, Codex, **goose**, anything speaking **Agent Client Protocol**) plug into the same project identity, permissions, and history.

Block’s own framing: models can already do the work; the bottleneck is **coordination**. Private agent windows made individuals faster and **teams slower** (humans as middleware ferrying context into Slack). Buzz keeps conversation, decisions, and work product together.

### 2.2 Technical highlights (beyond the landing page)

| Area | Design |
|------|--------|
| Identity | Keypair per human and **per agent**; owner signs scoped authorization; agent signs its own work (**authorization ≠ authorship**) |
| Protocol | Nostr for durable signed history; ephemeral encrypted telemetry/control paths |
| Agents | ACP harnesses; multi-agent swarms (expensive orchestrator + cheap workers) coordinating via channel mentions |
| Git | Object-storage-backed forge for **agent-scale concurrent pushes** (immutable packfiles + CAS manifest pointer; TLA+ model checking mentioned) |
| Device pairing | Formal security model for moving identity across devices |
| Open source | github.com/block/buzz — protocol specs, test vectors, security sections published |

### 2.3 Broader Block narrative

Separate from Buzz-the-product, Dorsey/Block have pushed **AI replacing hierarchical coordination** (“company as intelligence,” fewer middle-management status roles, tools like **goose** as internal agent harness). Buzz is the **collaboration OS** for that story.

### 2.4 Positioning vs AI Manager

| | Buzz | AI Manager |
|--|------|------------|
| Primary surface | Chat + Git + agents | Background context graph + (later) status ledgers |
| Success metric | Better multiplayer agent orchestration | Meetings deleted / focus time reclaimed |
| Data | What happens *inside Buzz* | Developer exhaust *from GitHub/Jira/Slack as sources* |
| Code / PRs | First-class forge | Explicitly **stripped** from product scope |
| Enterprise search | Not Glean | Anti-Glean strip |

**Threat:** Buyers say “we already have Buzz for agents.”  
**Response:** Buzz doesn’t give ACL-safe multi-hop **project truth** over existing tools; we do—and we don’t force a chat migration.

---

## 3. Centaur — deep brief

### 3.1 What it is

Centaur is Paradigm/Tempo’s open-source **production control plane for shared AI agents**: multiplayer, self-hosted, Slack-first. Used internally since ~Jan 2026 across investing, eng, design, recruiting, support.

### 3.2 Architecture (copyable pieces)

| Component | Role |
|-----------|------|
| Slackbot | Thin webhook → control plane |
| API (FastAPI) | Session lifecycle, tool REST, durable workflows |
| Postgres | Sole durable state (threads, checkpoints, audit) |
| Sandbox (K8s) | 1 thread = 1 container; warm pool; harness CLI inside |
| **Firewall (iron-proxy)** | Egress-only path; **inject secrets in-flight**; never give agent raw keys |
| Secrets manager | Isolated; not mounted into sandbox |
| Tools / Skills / Workflows | Org “userspace” overlays without forking kernel |
| Observability | Structured logs; default Victoria* stack tools |

### 3.3 “How secure is Centaur?” — credential injection (detail)

**Problem they call out:** laptop agent stacks dump API keys into **environment variables**; fine for personal use, catastrophic when the agent has Slack/GitHub/cloud/finance access (prompt injection → exfiltrate env).

**Their model:**

1. Tool declares needed hosts + secret *names* (e.g. talks to `api.slack.com`, needs `SLACK_BOT_TOKEN`).  
2. Sandbox starts with **no real secrets** in env/disk/memory.  
3. All egress forced through **iron-proxy**.  
4. Proxy matches **destination host (+ tool)** → fetches secret from vault → injects into request headers.  
5. Agent sees success; **never sees the token**.  
6. Network policy: sandbox cannot reach secrets service or open internet except via proxy.  
7. Egress fully logged; LLM responses scanned/redacted for leaked secret material.

This is the same industry pattern as credential brokering / egress inject (Anthropic managed agents, Vercel sandbox header inject, Cloudflare outbound workers, Envoy sidecars).

### 3.4 Positioning vs AI Manager

| | Centaur | AI Manager |
|--|---------|------------|
| Primary surface | Slack agent that *does* work for hours/days | Context plane for eng status & reasoning |
| Secrets | Network-edge injection | Today: env/config (must improve for agents) |
| Sandboxes | Core product | Stripped from V1 cost model |
| Context graph | Not the product | **V2 core** |
| Status meetings | Indirect (agent helps) | **Direct product goal** |

**Threat:** Security-conscious buyers ask “how do agents never see keys?” before buying any write path.  
**Response:** Adopt the **pattern** for our outbound tools/agents; stay the **context** product, not a Centaur fork.

---

## 4. Comparative matrix (full)

| Dimension | **AI Manager** | **Buzz** | **Centaur** | **Glean** (incumbent) |
|-----------|----------------|----------|-------------|------------------------|
| Job-to-be-done | End status theater via passive eng context | Multiplayer human+agent workspace | Secure shared agent runtime | Company-wide search + chat |
| Core asset | Org Context Graph + telemetry ACL | Signed rooms + Git history | Sandbox + tools + iron-proxy | Full-text + enterprise graph |
| Agents | Background twins (roadmap) | First-class | First-class | Portal agents |
| Credentials | ACL on data; secrets story evolving | Agent keys + scoped auth | Egress inject | Source ACLs + Protect |
| Self-host | Possible; mid-market SaaS lean | OSS self-host | OSS self-host K8s | Enterprise VPC |
| Cost thesis | Strip search/OCR/vector/code index | OSS; heavy workspace | OSS; K8s ops | High TCO |

---

## 5. What they do that we don’t (honest gaps)

1. **Multiplayer agent UX** (Slack room / Nostr room).  
2. **Long-running sandboxed execution** with real tools.  
3. **Credential-safe egress** as a first-class subsystem.  
4. **Durable multi-step workflows** (Centaur) / **agent-scale Git** (Buzz).  
5. **Signed agent identity** separate from human identity (Buzz).  
6. **Harness marketplace** (ACP/Goose/Codex) as default.

## 6. What we do that they don’t (moat)

1. **Metadata-only, low-privilege ingest** of existing Git/Jira/Slack exhaust without replacing Slack/GitHub.  
2. **Query-time ACL mirroring** on every event and graph edge (V1/V2).  
3. **Temporal lineage graph** purpose-built against the “relational reasoning wall” (Glean-class search dumps).  
4. **Anti-engagement economics**: measure success by meetings removed, not chat engagement.  
5. **Strip TCO**: no full-text index, no code search, no enterprise crawl.

---

## 7. Strategic recommendations

1. **Narrative:** Position AI Manager as the **permissioned engineering context layer**. Buzz/Centaur are **execution/collaboration layers**. Complementary, not identical.  
2. **Product:** Keep V2 graph quality high; expose graph later as **MCP/API for any agent** (including Buzz/Centaur/Goose).  
3. **Security:** Implement **egress credential injection** before any customer-facing write agent (V3). Harden V1 outbound backfill similarly.  
4. **Do not:** Rebuild Buzz or Centaur as our core.  
5. **Watch:** iron-proxy / Envoy inject patterns; ACP; whether buyers demand self-host agent runtime bundled.

---

## 8. Sources & further reading

- Block Engineering: *Buzz!* — https://engineering.block.xyz/blog/buzz  
- Buzz product / code — https://buzz.xyz · https://github.com/block/buzz  
- Paradigm: *Open Sourcing Centaur* — https://www.paradigm.xyz/writing/open-sourcing-centaur-multiplayer-self-hosted-secure-agents  
- Centaur docs / code — https://centaur.run · https://github.com/paradigmxyz/centaur  
- Goose / ACP ecosystem — https://goose-docs.ai · Agent Client Protocol  
- Credential brokering pattern — Infisical “Credential Brokering for AI Agents”; Envoy sidecar inject writeups; Cloudflare/Vercel sandbox egress  

---

*Living document: update when Buzz/Centaur ship material security or graph-like features.*
