# Session Handoff — Context Transfer 2026-08-04

**Repo:** `neeljoshi18/AI-Manager` · **`main`**  
**Staging:** https://status.neel.world/app/  
**Doctrine:** Data is currency. Gas not brakes. No hotspot asks (Actions). No auto-compact — handoff instead.

---

## 0. Sales status (read first)

**GO for test / design-partner calls** after sales smoke green.

| Bar | Status (2026-08-04) |
|-----|---------------------|
| Stack health | Live |
| Dual digests content | 2/2 |
| multi_person_ready | true |
| Graph story (PR+intents) | Live + hierarchical UI |
| My status → real drafts | Shipped (click person on Today) |
| Anti-spam | Notify Policy v1 |
| Partner docs | One-pager + install runbook |

**Not yet:** high-volume cold outbound proof; organic second-human GH commits (optional).

---

## 1. Paste prompt (next session)

```
You are continuing AI Manager monorepo neeljoshi18/AI-Manager main.

Read first:
1. starting-out-documents/Session Handoff_ Context Transfer 2026-08-04.md
2. plans/2026-08-04_sales-call-go.md
3. plans/2026-08-03_sales-call-readiness.md
4. plans/2026-08-03_data-currency-and-dev-insights.md

Doctrine: data is currency; Actions deploy not hotspot; gas not permission-brakes; no Linear/training yet.

Mission: keep staging sales-demo airtight. Run ./scripts/sales_smoke.sh. Fix only call-killing bugs. Soft outreach support for founder. Do not auto-compact — handoff at ~380–400k.

Start: smoke staging, confirm pilot_readiness + My status draft open path, then next highest-leverage sales polish only.
```

---

## 2. Attach list

1. `starting-out-documents/Session Handoff_ Context Transfer 2026-08-04.md`  
2. `plans/2026-08-04_sales-call-go.md`  
3. `plans/2026-08-03_sales-call-readiness.md`  
4. `scripts/sales_smoke.sh`  
5. `vertical-3/app-static/app.js` + `index.html` + `styles.css`  
6. `scripts/github_live_bridge.py`  
7. `deploy/docker-compose.app.yml` + `.github/workflows/deploy-staging.yml`  
8. `starting-out-documents/Design Partner_ One-Pager.md`  
9. `starting-out-documents/Design Partner_ Install Runbook.md`  
10. `starting-out-documents/Human Demo Script.md`  

---

## 3. Shipped this arc (08-03 → 08-04)

- Commit poller + Dev insights  
- Commit messages on graph  
- Dual digests seed + real 2/2 content  
- Hierarchical graph (no hairball) + graph_story  
- Sales polish: tenant unity, My status real drafts, readiness strip, cache-bust, CSS toolbar, sales_smoke.sh  
- **Graph snapshot fix:** rank Person/Repo/PR/Intent before Commit so trunc doesn't starve edges  
- Deploy V1 restart loop until healthy  


---

## 4. Context discipline

- Soft stop ~**380–400k**; hard preference: **new session + this handoff**, not auto-compact.  
- Prefer worktree `/tmp/ai-manager-work/AI-Manager` if Desktop TCC blocks.  

---

## 5. Ops

```bash
git push origin main                          # Actions deploy
gh workflow run "Deploy staging" -f skip_build=true
./scripts/sales_smoke.sh
```

Volumes sacred — never `docker volume prune` on staging data.
