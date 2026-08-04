# Auth, Onboarding & Chat Delivery

**Purpose:** Answer login, Connect buttons, Slack vs Teams, and multi-chat horizon without overpromising.

---

## 1. Three planes (keep these separate)

| Plane | What it is | Examples |
|-------|------------|----------|
| **Identity** | Who is on the tenant | Google/SSO, invite seats, roles |
| **Work ingest** | What engineers ship | **GitHub App** (PRs, commits, pushes) |
| **Delivery** | Where digests land | **Slack** bot · **Teams** bot · later other chat |

Google does **not** replace GitHub. Chat does **not** replace the graph.

---

## 2. Architecture (adapter model)

```
GitHub ingest → V1 events → bridge → V2 graph → V3 digests
                                      ↓
                              Notify Policy v1
                                      ↓
                         Delivery adapter interface
                    ┌─────────────┼─────────────┐
                 Slack bot    Teams bot     Future chat
                 (live)       (roadmap)     (WhatsApp / …)
```

- Same digest text and actions: **Approve · Edit · Don't send**  
- Bot tokens only in **egress vault** (never twin-api env)  
- **Never** silent-read private human↔human messages on any channel  

---

## 3. Chat options

### Slack (primary today)

- Bot DMs mapped users  
- Map: GitHub identity → Slack user id  
- **Demoable** on staging  

### Microsoft Teams (roadmap — same product)

**Sales line:** *“If you’re on Teams, same loop—connect Teams instead of Slack. Digests in Teams; Approve / Edit / Don't send.”*

| Item | Detail |
|------|--------|
| App | Azure Bot + Teams app manifest |
| Identity | Azure AD user / UPN |
| Map | GitHub → Teams/AAD id |
| UI | Adaptive Cards for actions |
| Status | **Not live** on staging yet |
| Order | After Slack path is airtight; shared delivery interface |

### Horizon (only if asked)

| Channel | Stance |
|---------|--------|
| WhatsApp Business / other enterprise chat | Adapter architecture allows it **after** Slack+Teams prove digests |
| Email | Emergency fallback only — not primary eng status |

Do **not** promise WhatsApp dates on first calls.

---

## 4. What “Connect” grants (honest)

| Connect … | Grants | Does not grant |
|-----------|--------|----------------|
| **Slack / Teams** | Message mapped users; resolve chat ids | Surveillance of private 1:1 history; full chat search index |
| **GitHub App** | Repo events for installed repos | Access to unrelated orgs; replace code review tools |
| **Google / SSO** | Tenant membership + roles | Eng graph data by itself |

---

## 5. Onboarding options (effort)

### Option 1 — Manual (live now)

Champion/founder pastes chat user ids + GitHub logins; webhook + bot in vault.

| Pros | Cons |
|------|------|
| Works today; 45–90 min | Feels ops-heavy |
| Full control | Not self-serve 10 seats |

**Sales:** “White-glove pilot—we map your pod with you.”

### Option 2 — Connect Slack or Teams + Connect GitHub

Install buttons; auto identities for mapping.

| Pros | Cons |
|------|------|
| Product feel | Needs finish + Teams adapter |
| Less pasting IDs | Still need tenant isolation |

**Effort:** Medium (Slack polish first; Teams next).

### Option 3 — Google/SSO + connectors

Company login for seats; then chat + GitHub.

| Pros | Cons |
|------|------|
| Clean “company login” story | Extra IdP; multi-tenant packaging |
| Roles champion vs member | Harder than Option 2 alone |

**Effort:** Medium–high.

---

## 6. Recommended sequence

1. **Manual map** — always keep (pilots, air-gap, speed).  
2. **Connect Slack + Connect GitHub** — primary product UX.  
3. **Connect Teams** — parity adapter (same digests/actions).  
4. **Google/SSO** — invites + roles with multi-customer packaging.  
5. **Other chat** — only after Slack+Teams stable.  

**Sales today:**  
*“We white-glove connect your chat (Slack or Teams) and GitHub and map your eng pod. Roadmap is Connect buttons so the champion doesn’t paste IDs forever.”*

---

## 7. GitHub App (champion installs once)

- One install on the **org/repos** that should feed status  
- Webhook → product host `/v1/tenants/<tenant>/webhooks/github`  
- Not “each developer grants personal Slack access to GitHub”  

---

## 8. Multi-chat Q&A cheat sheet

| They say | You say |
|----------|---------|
| We’re on Slack | Perfect—that’s live for pilots. |
| We’re on Teams | Same product design; Teams delivery is on the roadmap with the same Approve loop. Pilot can white-glove or start with Slack if both exist. |
| We need WhatsApp | Adapter model supports more channels later; we prove Slack+Teams first. |
| Can you read all our Teams history? | No. We deliver digests and resolve identity—we don’t wiretap private chats. |
