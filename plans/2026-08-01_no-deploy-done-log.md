# No-deploy done log (do not re-do)

**Purpose:** Track airtight work shipped on `main` while waiting for hotspot/Actions deploy.  
**Update:** Append only when a slice lands. Agents: **read this before starting** so we don’t rework.

---

## Already done (prior sessions)

| When | Slice | Commit tip |
|------|--------|------------|
| 2026-07-31 | Notify Policy v1, multi-person map, prune, graph collapse UI | pre-handoff |
| 2026-07-31 | Commits/pushes in digests, neighborhood soft-fail | `94f8f15` |
| 2026-07-31 | Multi-identity compile (gu_* aliases) | `8a7f2c6` |
| 2026-07-31 | Session handoff + Actions deploy workflow | `89b4500` / `5dc1d08` |
| 2026-07-31 | A2 24h lookback + valid_from + dual-person tests | `8088edc` |
| 2026-08-01 | Server-side hide demo seed + pulse demo split + partner package | `9960387` |

---

## This loop (2026-08-01 polish → deploy-quality)

| # | Slice | Why | Status |
|---|--------|-----|--------|
| 1 | **Prune folds gu_* into keeper** | Without merge, multi-identity digests die after prune | Done |
| 2 | **Scheduler / ensure_users seed gu_* membership** | Team compile did; scheduler did not → empty neighborhoods | Done |
| 3 | **Team member alias merge (not replace)** | Bridge re-upsert must not clobber historical gu_* | Done |
| 4 | **Fix empty→items draft upgrade (real bug)** | Fallthrough called `put_draft` → Conflict; upgrades never DM’d | Done + test |
| 5 | **`GET …/pilot_readiness`** | Machine A1–A7 go/no-go after deploy | Done |
| 6 | **Onboarding multi_person + digests_with_content** | Unique Slack among person twins; A2 step | Done |
| 7 | **Team last_digest has_content / approx_item_count** | A2 board readability | Done |
| 8 | **UI Today/Team digest content labels** | Show empty vs has items | Done |
| 9 | **Config / membership unit tests** | 24h default, lookback, alias helpers | Done |

---

## Still blocked (do not fake as done)

| Item | Blocker |
|------|---------|
| Staging runs latest main | Hotspot or Actions `STAGING_*` secrets |
| Live A2 dual non-empty digests | Deploy + paneerjeera (or 2nd human) GH edges |
| Soft outreach to strangers | A2 live green only |

---

## Satisfactory bar for “stop iterating this arc”

- [x] Empty→items upgrade works (tested)  
- [x] Prune preserves multi-identity aliases  
- [x] Scheduler membership includes gu_*  
- [x] Pilot readiness endpoint for post-deploy smoke  
- [x] Partner package + soft-outreach checklist current  
- [x] Demo seed not product theater (server-side)  
- [ ] **One deploy + smoke** (founder)  

When founder says **deployed**: run handoff smoke + `GET /v3/tenants/ten_github/pilot_readiness` and mark A2 live.

---

## 2026-08-03 — Deploy + campus path

| Item | Status |
|------|--------|
| Hotspot SSH deploy of main `25d1981` + twin/v2 rebuild | Done |
| `STAGING_HOST` / `STAGING_USER` / `STAGING_SSH_KEY` via `gh secret set` | Done |
| Actions workflow post-deploy smoke | Done (this commit) |
| `deploy/scripts/deploy_from_ci_or_hotspot.sh` | Done |
| sshd :2222 | Not set (sudo password on droplet) — Actions is the campus path |

Campus forever: `git push origin main` → GitHub Actions Deploy staging.

## 2026-08-03 durability confirm (live)

| Check | Result |
|-------|--------|
| Actions deploy `30783635161` | **success** |
| V2 load on boot | `loaded embedded V2 graph snapshot nodes=14 edges=17` |
| `v2_graph.json` | **13611 bytes** on volume |
| `v2_membership.json` | **517 bytes** after ensure_users |
| Graph after full rebuild | **nodes still present** (not wiped) |
| Digests | neeljoshi18 **4 items**; paneerjeera empty (no edges) |
| Fast deploy path | `deploy_fast.sh` + BuildKit caches + sequential Actions builds |
