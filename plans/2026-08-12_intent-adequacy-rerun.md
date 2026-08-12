# Intent Adequacy Re-run — 2026-08-12

**Pack:** `plans/packs/2026-08-12_ten_github_neeljoshi18.json`  
**Baseline:** `plans/2026-08-06_intent-adequacy-experiment.md`  
**Tenant / subject:** `ten_github` / `neeljoshi18`  
**As of:** 2026-08-12T19:20:22Z

## Verdict vs baseline

| Question | 2026-08-06 | 2026-08-12 |
|----------|------------|------------|
| Work / trajectory profile | **Yes** | **Yes** (stronger) |
| Trustworthy intent profile (organic claims) | **No** | **Partial — yes** |
| Real intent conflicts for team | **No** (5/5 demo) | **Honest empty** (0 live, 9 demo, `empty_reason=only_demo_seeds`) |
| PR starvation | 1 PR / 237 commits | **13 PR / 250 commits** |
| Organic typed intents | 0 | **12+ github_pr** live ledger |

**One-line:** Trajectory remains strong; **claims layer unlocked** via PR poller; conflicts correctly show **only demo seeds** with an honest empty live state.

## Pack summary numbers

| Metric | Value |
|--------|------:|
| Graph nodes (insights) | 308 |
| Commit nodes | 250 |
| PullRequest nodes | **13** |
| Intent nodes | 20 |
| Person nodes | 11 |
| Repo nodes | 14 |
| neeljoshi18 authored (activity) | 214 |
| Team mapped / multi_person | 2 / true |
| Pulse live conflicts | **0** |
| Pulse demo conflicts | **9** (`empty_reason=only_demo_seeds`) |
| Pulse live intents sample | **12** (demo_tagged 0 in pack filter) |
| Pack errors | 0 |

## Score revision (0–5)

| Signal | Profile 06 | Conflicts 06 | Profile 12 | Conflicts 12 |
|--------|----------:|-------------:|----------:|-------------:|
| Commits trajectory | 4 | 1 | **5** | 1 |
| Typed organic intents | 1 | 2 | **3** | 2 |
| PR / review graph | 1 | 1 | **3** | 2 |
| Conflict cards live | 1 | 2 | 1 | **3** (engine + honesty) |
| Demo hygiene digests | 1 | — | **4** (new compile blockers=0) |
| Slack channel claims | 0 | 0 | 0 | 0 |
| CI checks | 0 | 0 | **1** (projection path + PR status poll) |

**Adequacy score (honest):** trajectory **5/5**, claims **3/5**, conflicts **2/5** (engine ready; organic dual-owner friction still rare).

## Digest dogfood (same session)

| Check | Result |
|-------|--------|
| Fresh compile `blocker_count` | **0** (seed BLOCKS stripped) |
| Confidence rollup | **medium** (not “blocked”) |
| Preview text | “work in progress” — no “Blocked via BLOCKS” |
| Historical published drafts Aug 5–6 | Still show seed blockers (immutable history) |
| Seed story-1 PR titles in items | Filtered in twin-compiler (this ship) |

## Success criteria from experiment §5.2

| DoD | Status |
|-----|--------|
| ≥1 organic open intent per mapped eng **or** explicit empty | **Met** for neel (github_pr + profile live intents) |
| ≥1 non-demo conflict / week **or** clean empty | **Met** — empty_reason `only_demo_seeds` |
| Follow-through on claims >72h | **Partial** — API returns supported on organic SHIP/FIX |
| Pack demo hygiene | **Improved** — live pulse conflicts demo-tagged properly |
| Champion can explain without standup | **Partial** — open focuses + promises work; dual-owner live friction still rare |

## Residual gaps

1. Slack Events live density (bot in channel) — human install  
2. Organic **live** conflict (two humans disagree on same real PR) needs dual-owner work  
3. Historical digests remain contaminated (do not rewrite history; new compiles clean)  
4. Optional env: morning digest channel / Linear  
