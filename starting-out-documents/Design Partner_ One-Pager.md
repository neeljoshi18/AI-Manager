# Design Partner — One-Pager

**Product:** AI Manager — permissioned engineering **context plane** + **meeting elimination**  
**Staging:** https://status.neel.world/app/  
**Not:** Glean search · Buzz workspace · Centaur agent OS · silent private 1:1 wiretaps

**Pitch (use this):** Status that writes itself from your PRs and pushes — you approve before anyone sees it.  
**Alt:** Kill the standup. Keep the signal.

---

## What you get (2-week shadow)

| You connect | We do | You control |
|-------------|--------|-------------|
| GitHub (App/webhooks) | Continuous ingest → graph (PRs, commits, pushes) | ACL stays on sources |
| Slack (bot via vault) | **Rare** status digests (~24h lookback; change-only + daily cap) | **Approve / Edit / Don't send** |
| Team map (2+ people) | Person twins + blocker/conflict cards | Who is mapped |

---

## What we never do

- Index your Drive/wiki as a search product (anti-Glean)
- DM on every webhook or every open PR every 30 minutes (ADR-014 + Notify Policy v1)
- Read private human↔human Slack DMs without bot/opt-in (ADR-015)
- Put bot tokens in worker env (egress vault only, ADR-012)
- Rank engineers by LOC or “productivity scores”

---

## Success criteria (your team)

1. **≥2 humans** mapped GitHub → Slack (`multi_person_ready`)  
2. Real PRs/commits appear in digests without manual copy-paste  
3. At least one **blocker/conflict** visible in product UI when it exists  
4. **Approve**, **Edit**, or **Don't send** at least once (loop works)  
5. Optionally **cancel one standup** if digests are trustworthy enough  

---

## Time cost

| Role | Effort |
|------|--------|
| Champion eng | ~1–2h setup (see Install Runbook); ~5 min/day glance |
| Rest of team | Receive digests; optional edit |

**Install:** `starting-out-documents/Design Partner_ Install Runbook.md`

---

## After 10–14 days

- Scorecard: DMs sent, suppressions, Don't-send rate, empty windows, standups canceled  
- Optional: export **structured gold** (approved digests + intent labels) for later **customer-prem** model SKU (ADR-016) — **not** required for beta value  
- Choose: stay rules/cloud path, or enable local Model Router when ready  

---

## Ask

*“Two weeks of shadow digests. You cancel one standup if Don't-send rate is healthy. We never silent-read private 1:1s.”*

Contact: founder-operated install. Secrets stay in your vault path or our staging vault under your control for pilot only.
