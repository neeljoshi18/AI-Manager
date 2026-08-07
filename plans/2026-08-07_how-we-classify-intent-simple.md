# How we classify intent today (simple words)

## What we do under the hood

We look at **words and work signals**, not personality.

1. **GitHub motion** — commits, PRs, freeze labels → technical tags (`SHIP`, `BLOCKED`, …).  
2. **Chat phrases** — “blocked on…”, “do not merge…” → same tags.  
3. **Explicit capture** — someone types a claim in the app or bot.  
4. **Commitments (new)** — “I’ll send…”, “I will fix by Friday” → **open loops** (who owes what).  
5. **Conflicts** — two tags fight (ship vs hold) → **friction alert**.

Those tags are for **machines**. Execs and champions should see **plain English**.

## What people should see

| Under the hood | Plain English |
|----------------|---------------|
| `BLOCKED` | “Neel is stuck waiting on security review.” |
| `FREEZE` | “Hold the merge until after the demo.” |
| `SHIP` | “Aiming to ship the Neon graph export.” |
| Open commitment | “Neel owes the team: send the deck by Friday.” |
| Conflict ship_vs_freeze | “Mixed signals — one person wants to ship, another wants to hold.” |
| Commit volume | “Lots of code motion this week — turn it into a short status story.” |

## Commitments vs Jira/Linear

| | Jira / Linear | Our commitments |
|--|---------------|-----------------|
| What | Tickets / issues | Human **promises** |
| Created when | You file or automate (slash, emoji) | Chat says “I’ll…” / explicit capture |
| Closed when | You move status | Chat says done/shipped/sent **or** you mark done |
| Best for | Full issue tracking | Teams living in Slack; light accountability |

**Atlassian/Linear do not** fully solve ambient “detect promise → track → auto-resolve from conversation.” Integrations create tickets when asked; they don’t own the commitment loop as product core.

## Inspired by

- **Commit** (Miten) — promises both ways, act-on-today, quiet when done  
- **Minimi** — open loops that close when work shows resolution  

We adapt that for **team Slack + champion cockpit**, not personal WhatsApp.