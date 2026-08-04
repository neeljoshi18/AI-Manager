# AI Manager — Sales Call UX Brief

**Audience:** Founder on design-partner / soft test calls  
**Staging:** https://status.neel.world/app/  
**Pitch:** Status that writes itself from your PRs and pushes — you approve before anyone sees it.

---

## 1. What this product is

AI Manager is a **permissioned engineering context plane** plus **meeting elimination**.

| We do | We are not |
|-------|------------|
| Ingest GitHub work (PRs, commits, pushes) under ACL | Glean-style company search |
| Map people ↔ work ↔ intents/conflicts on a graph | Buzz workspace / chat OS |
| Rare private status digests in chat | Centaur agent sandbox OS |
| Human gate: **Approve / Edit / Don't send** | Silent private 1:1 chat wiretap |
| Kill standups with trusted signal | LOC / productivity rankings |

Success metric: **meetings deleted**, not messages sent.

---

## 2. How a ~10-person team “gets on the product” (today)

**Honest pilot model: white-glove install + Slack-first for ICs.**

There is **no** “each employee creates a website password” flow yet. People already live in:

- Company **GitHub** (or repos you install the App on)  
- Company **Slack** (or later **Teams** — see doc 03)

### Access model today

| Who | How they join | Primary surface |
|-----|---------------|-----------------|
| **Champion** (eng manager / lead) | You map them + share pilot URL | Web app `/app/` |
| **Mapped ICs** (start 2, grow to 10) | GitHub work + chat user id on Team map | **Private digests in chat** |
| **Unmapped people** | Nothing yet | Invisible to digests (by design) |
| **You (vendor)** | Install runbook | Setup, health, compile |

### DNS / multi-customer (say this)

- **Pilot:** shared host (`status.neel.world`) or one dedicated instance you run for them.  
- **Not yet:** self-serve `acme.ai-manager.com` signup.  
- Custom domain is packaging later; **value is digests + graph**, not DNS.

### Tenant note

Internal id on staging is `ten_github` — ops detail, not something customers invent.

---

## 3. Employee experience (most of the 10)

**Primary surface: private chat message (Slack today).**

When the story of their work **changes** (Notify Policy v1 — change-only + daily cap):

1. They receive a **private** status draft (lookback of recent work — product default ~24h).  
2. Bullets are **evidence-backed** (PRs/commits/pushes — not invented).  
3. They act:

| Action | Meaning |
|--------|---------|
| **Approve** | Accurate — share / publish path OK |
| **Edit** | Fix the words first |
| **Don't send** | Kill this draft; never post |

**They do not get by default:**

- Other people’s digests as a surveillance feed  
- LOC leaderboards  
- A ping on every webhook  
- Bot reading private human↔human DMs  

**Feeling to sell:** “It already knows my open work. I’m not typing standup bullets.”

---

## 4. Champion / management experience

**Primary surface: web app** (pilot URL).

| Screen | Use on the call |
|--------|-----------------|
| **Today** | Readiness, team digests board, blockers/conflicts |
| **Team** | Map people, compile digests, multi-person ready |
| **My status** | Open a real draft → Approve / Edit / Don't send |
| **Graph** | People → work → intents (SHIP / FREEZE / BLOCKED) |
| **Connections** | Stack health |
| **Settings** | Anti-spam: suppressions ≫ sends |
| **Dev insights** | Optional dogfood heat (team activity) |

**Champion does not get:** individual stack-ranks, full company knowledge search, agents auto-merging code.

**Feeling to sell:** “I see the map and blockers without a 15-minute theater round.”

---

## 5. Five-minute live demo path

1. Open https://status.neel.world/app/ (hard refresh if needed).  
2. **Today** — multi-person / digests board; click a person.  
3. **My status** — real draft + evidence; name Approve / Don't send.  
4. **Graph** — people, PR, intents (Hide demo on).  
5. **Settings** — notify policy; suppressed ≫ sent.  
6. Optional: phone screenshot of Slack DM.

**Avoid on call:** Lab console, alice/bob as “real team,” promising Linear/Drive search, promising live Teams if not built yet.

---

## 6. Hard questions (scripted)

| Question | Answer |
|----------|--------|
| How do people log in? | ICs are chat-first. Web is champion cockpit on pilot. Self-serve seats are roadmap. |
| Separate DNS for us? | Pilot host or dedicated instance; custom domain later. |
| We use Teams not Slack | Same product loop; Teams is the delivery channel on the roadmap (doc 03). Pilot may start on Slack or white-glove Teams when adapter ships. |
| Who sees my status? | You first, privately. Team only after Approve/publish path. |
| Can managers spy? | Work context + conflicts — not LOC rankings. |
| How many users day one? | Map **2** first; expand to the eng pod when digests are trusted. |
| Is this multi-tenant SaaS? | Design partner / founder-operated install today. |

---

## 7. Success in two weeks (their ROI)

1. ≥2 humans mapped with real digests when work changes  
2. Blocker/conflict visible when it exists  
3. Someone used Approve / Edit / Don't send  
4. Optionally **one standup canceled**  
5. Don't-send not 100% angry rejection; suppressions high vs spam  

**Ask:** *“Two weeks of shadow digests. Cancel one standup if Don't-send stays healthy. We never silent-read private 1:1s.”*

---

## 8. One-paragraph close

You don’t put ten people on a new website account. You connect GitHub and their chat once, map the eng pod, and people keep working where they already are. The system maintains a permissioned map of work and intents. When the story changes, each mapped engineer gets a private draft they can Approve, Edit, or Don't send. The champion uses the web app for multi-person digests, conflicts, and the graph—so the standup can die.
