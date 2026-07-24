# Design Partner — One-Pager

**Product:** AI Manager — permissioned engineering **context plane** + **meeting elimination**  
**Staging:** https://status.neel.world/app/  
**Not:** Glean search · Buzz workspace · Centaur agent OS · silent private 1:1 wiretaps

---

## What you get (2-week shadow)

| You connect | We do | You control |
|-------------|--------|-------------|
| GitHub (App/webhooks) | Continuous ingest → graph | ACL stays on sources |
| Slack (bot via vault) | **Batched** status digests (not every PR) | Veto / edit / publish |
| Team map (2+ people) | Person twins + conflict/blocker cards | Who is mapped |

**Pitch in one line:** Kill the status standup. Keep continuous signal. Humans veto.

---

## What we never do

- Index your Drive/wiki as a search product (anti-Glean)
- DM on every webhook (ADR-014: batch notify)
- Read private human↔human Slack DMs without bot/opt-in (ADR-015)
- Put bot tokens in worker env (egress vault only, ADR-012)
- Rank engineers by LOC or “productivity scores”

---

## Success criteria (your team)

1. **≥2 humans** mapped GitHub → Slack  
2. Real PRs appear in digests without manual copy-paste  
3. At least one **blocker/conflict** visible in product UI  
4. Veto or edit at least one draft (loop works)  
5. Optionally **cancel one standup** if digests are trustworthy enough  

---

## Time cost

| Role | Effort |
|------|--------|
| Champion eng | ~1–2h setup; ~5 min/day veto glance |
| Rest of team | Receive digests; optional edit |

---

## After 10–14 days

- Scorecard: DMs sent, veto rate, empty windows, standups canceled  
- Optional: export **structured gold** (approved digests + intent labels) for later **customer-prem** model SKU (ADR-016) — **not** required for beta value  
- Choose: stay rules/cloud path, or enable local Model Router when ready  

---

## Ask

*“Two weeks of shadow digests. You cancel one standup if veto rate is healthy. We never silent-read private 1:1s.”*

Contact: founder-operated install. Secrets stay in your vault path or our staging vault under your control for pilot only.
