# Plan: Credential Egress Proxy (Centaur-inspired)

**Date:** 2026-07-22  
**Status:** Approved pattern; **MVP implemented** (`vertical-security/` egress-proxy + V1 `EgressClient` / secrets load). Full vault backends + network policy still open.  
**Inspiration:** Centaur iron-proxy; industry credential brokering (Envoy inject, Cloudflare/Vercel sandbox egress, Infisical)  
**Decision:** Adopt the **security pattern**, not the full Centaur product.

---

## 1. Why we need this

### Problem
Any process that holds `GITHUB_TOKEN` / `SLACK_BOT_TOKEN` in environment variables can leak them via:

- Prompt injection (agent prints env or posts to attacker URL)  
- Log exfiltration  
- Compromised dependency  

Centaur’s answer: **agent never holds long-lived secrets**; a network egress proxy injects them after the request leaves the untrusted process.

### Why it fits AI Manager
- Our commercial story is already **low privilege + ACL + no code storage**.  
- Vertical 3 (status DMs, write tools) **cannot** ship enterprise without this.  
- Industry standard in 2026 — buyers will ask.

### Why not full Centaur
- We are not primarily a multiplayer Slack coding agent.  
- Full K8s sandbox fleet conflicts with V1 “strip PTC sandboxes” cost thesis unless scoped to a later agent vertical.  
- Keep Rust V1/V2; add a **small egress subsystem**.

---

## 2. Target architecture

```
┌─────────────────────────┐
│ Worker / future agent   │  HTTP client with PLACEHOLDER or no auth
│ (no real secrets in env)│
└───────────┬─────────────┘
            │ only route via proxy
            v
┌─────────────────────────┐
│ Egress Proxy            │  allowlist host + tool → secret ref
│ (Envoy or Rust)         │  inject Authorization / headers
└───────────┬─────────────┘
            │ fetch secret
            v
┌─────────────────────────┐
│ Secrets backend         │  Vault / Infisical / SOPS / KMS
└─────────────────────────┘
            │
            v
     External API (GitHub, Slack, …)
            │
            v
     Audit log + response secret-scan/redact
```

**Rules**
1. Untrusted code paths never receive raw long-lived tokens.  
2. Fail **closed** if proxy/vault unavailable (no silent fallback to env secrets).  
3. Inbound webhooks (V1 HMAC verify) are **not** proxied — only outbound.  
4. Tool registry declares: tool name → allowed hosts → secret keys.

---

## 3. Vertical impact

| Vertical | Impact | Priority |
|----------|--------|----------|
| **V1** | Medium — production secrets loading; outbound backfill/GitHub API clients via proxy | P1 before paid outbound |
| **V2** | Low — graph/CRDB only; only if enrichment workers call external APIs | P2 |
| **V3+ agents / status write** | Critical — all tool calls through proxy | P0 for V3 launch |
| **Future agent runtime** | Defines security boundary | With that vertical |

---

## 4. Implementation phases

### Phase S0 — Design freeze
- [ ] ADR-012 in Architecture Decision Log  
- [ ] Choose proxy tech: **Envoy ext_authz** vs minimal **Rust reverse proxy**  
- [ ] Choose secrets backend for dev (file/SOPS) vs prod (Vault/Infisical)  
- [ ] Tool registry schema (YAML/JSON)

### Phase S1 — MVP proxy (dev)
- [ ] Docker Compose service `egress-proxy`  
- [ ] Map `api.github.com` → `GITHUB_TOKEN` from vault  
- [ ] Unit tests: inject, deny unknown host, redact  
- [ ] Sample Rust HTTP client that routes via `HTTPS_PROXY` / custom transport

### Phase S2 — Vertical 1 integration
- [ ] Production mode: load tenant webhook secrets from vault (not `.env` files in deploy)  
- [ ] Any GitHub/Jira REST backfill worker uses proxy  
- [ ] TC-S01…S04 (see below)  
- [ ] Regression: webhook ingest TC-01…06 still pass

### Phase S3 — Vertical 2
- [ ] Confirm no outbound secrets on happy path  
- [ ] If projector gains external enrichers → force proxy  
- [ ] Document `V1_COCKROACH_URL` DB auth separately (IAM/short-lived preferred later)

### Phase S4 — Vertical 3 readiness
- [ ] All write tools (Slack DM, etc.) only via proxy  
- [ ] Optional sandbox + network policy “cannot bypass proxy”  
- [ ] Security review checklist for customers

---

## 5. Test matrix (when implementing)

| ID | Scenario | Pass |
|----|----------|------|
| TC-S01 | Worker process environment listing | No raw long-lived API tokens |
| TC-S02 | Allowlisted host call | Upstream sees injected credential; worker never logged it |
| TC-S03 | Non-allowlisted host | Connection refused / 403 at proxy |
| TC-S04 | Response body contains secret pattern | Redacted in logs; not returned to agent if scanned |
| TC-S05 | Proxy down | Fail closed; no env fallback |
| TC-S06 | V1 webhook HMAC ingest | Unaffected (inbound) |
| TC-S07 | V2 project + graph query | Unaffected |

---

## 6. What we will not do in this workstream

- Replace V1/V2 with Centaur’s FastAPI stack  
- Require full K8s for mid-market MVP (proxy can be a single container first)  
- Put webhook signing secrets through egress (wrong direction)

---

## 7. Coding agent handoff (when you say “implement proxy”)

Suggested order:
1. Scaffold `vertical-security/` or `vertical-1/crates/egress-proxy` + compose.  
2. Tool registry + inject logic + tests TC-S01–S05.  
3. Wire one V1 outbound client as reference.  
4. Document ops in README; update decision log ADR-012 to “Implemented”.

---

## 8. References

- Paradigm Centaur “How secure is Centaur?” — iron-proxy inject  
- https://iron.sh (referenced by Centaur)  
- Infisical: Credential Brokering for AI Agents  
- Envoy sidecar credential inject patterns  
- Cloudflare / Vercel sandbox outbound credential injection  

---

*Do not start coding until product explicitly prioritizes this workstream.*
