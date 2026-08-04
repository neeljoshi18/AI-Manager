# Employee Mapping Playbook (Sales Q&A)

**Question this answers:** “How does each employee get onto AI Manager?”

**Outcome of every path (same product objects):**

- Person twin (stable subject id)  
- GitHub provider aliases (login / numeric id)  
- Chat user id (Slack **or** Teams)  
- ACL groups so their graph neighborhood compiles  

Only **mapped** people get digests. Unmapped = no spam (by design).

---

## Rule of thumb for a 10-person team

1. **Start with 2** airtight digests (champion + one IC).  
2. Expand map to the rest of eng when Don't-send rate is healthy.  
3. Never map whole company chat (Sales, HR, etc.) on day one.  

---

## Path A — Champion maps roster (manual) · **Live today**

**How**

1. List of engineers: name, GitHub login, Slack or Teams user id.  
2. Enter in **Team** UI (or env `SLACK_USER_MAP` / equivalent for pilot).  
3. After GitHub activity, digests compile for those with work.  

**Sales line:** *“Fastest pilot: we map your 10 with you in one install call.”*

**Effort:** Low tech risk; 45–90 min install.

---

## Path B — Connect chat → auto-suggest · **Vision / next**

**How**

1. Connect Slack or Teams.  
2. Pull workspace members (name + chat id).  
3. Champion **ticks eng pod** (not entire company).  
4. Match GitHub collaborators by email/login; flag unmatched.  

**Sales line:** *“Connect chat once; you choose who gets digests—no spam to all of Sales.”*

---

## Path C — Employee self-map (invite) · **Vision**

**How**

1. Invite link → Google/SSO or “Sign in with Slack/Teams.”  
2. Employee links GitHub (OAuth or typed login).  
3. Lands as **member**; champion can disable.  

**Sales line:** *“Each person can join via invite and link GitHub—champion still controls the pod.”*

---

## Path D — GitHub-first · **Vision + partial today**

**How**

1. GitHub App sees collaborators / recent commit authors.  
2. Suggest people already shipping.  
3. Attach chat id second (manual or Connect).  

**Sales line:** *“We can start from who’s already shipping in the repo.”*

---

## Comparison

| Path | Best for | Status |
|------|----------|--------|
| A Manual | Design partner, speed, control | **Live** |
| B Auto-suggest from chat | Mid-size pods, less pasting | Roadmap |
| C Self-invite | Self-serve feel, distributed teams | Roadmap |
| D GitHub-first | Repo-centric teams | Partial / roadmap |

All four can coexist. **A never goes away.**

---

## Scripted answers

| They ask | You answer |
|----------|------------|
| Does every employee create a website account? | Not required. ICs are chat-first. Mapping links GitHub + chat identity. |
| Do we paste 10 Slack IDs? | Pilot often yes (Path A)—fast and safe. Product path is Connect + tick roster (B) or invite (C). |
| What if someone isn’t mapped? | No digest, no spam. We expand when ready. |
| Can the whole Slack workspace join? | You select the eng pod only. |
| Teams users? | Same mapping model: GitHub ↔ Teams/AAD user id (adapter roadmap). |
| How long for 10 people? | 2 people in first install call; rest over the learning window. |

---

## Mapping quality checklist (install)

- [ ] ≥2 people with chat id + GitHub alias  
- [ ] Multi-person ready  
- [ ] Each has recent work **or** honest empty window (no fake DM)  
- [ ] Graph shows real people (not demo alice/bob as the story)  
- [ ] Compile: with_items when work exists; empty_reason when not  

---

## One-liner close

*“Mapping is how we stay permissioned and spam-free: only the eng pod you choose gets digests. Start with two, prove trust, then bring the other eight.”*
