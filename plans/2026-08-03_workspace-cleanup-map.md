# Workspace cleanup map (2026-08-03)

## Where the ~20 GB actually was

| Location | Before | After tranche 1 | Safe to clear? |
|----------|--------|-----------------|----------------|
| **Docker Build Cache** (local Mac) | **21.67 GB** | **~4 KB** | Yes — rebuilds regenerate |
| Docker Images | ~2.7 GB | kept (local platform stack) | Only unused/dangling |
| Docker Volumes | ~2.3 GB (cockroach/clickhouse/minio local) | **kept** | **No** — data |
| `/tmp/ai-manager-work/AI-Manager` clone | 811 MB | ~230 MB after v1 target clean | targets yes |
| Source monorepo (git) | tiny (targets gitignored) | — | Never delete source |
| Staging droplet volumes | graph/twin journals | **never prune** | **No** |

**Desktop monorepo path** (`~/Desktop/ai-manager`) was **not accessible** to the agent (macOS privacy). If your Finder shows 20 GB there, run on your machine:

```bash
cd ~/Desktop/ai-manager
du -sh */target 2>/dev/null
rm -rf vertical-1/target vertical-2/target vertical-3/target vertical-security/target
docker builder prune -af   # if not already done
```

## Folder plan (do one per session)

| Order | Folder | Status |
|-------|--------|--------|
| 0 | Docker Build Cache (system) | **Done 2026-08-03** — freed ~21.7 GB |
| 1 | `vertical-1/` | **Done 2026-08-03** — removed `target/` (580 MB in work clone) |
| 2 | `vertical-2/` | **Done 2026-08-03** — removed `target/` (~228 MB work clone) |
| 3 | `vertical-3/` | **Done 2026-08-03** — already clean (~624 KB source; no `target/`) |
| 4 | `vertical-security/` | **Done 2026-08-03** — no target / already lean; secrets kept |
| 5 | `deploy/` | Pending — scripts/docs only; no big caches |
| 6 | `scripts/` + `plans/` + `starting-out-documents/` | Pending — docs only |
| 7 | Local `vertical-1` docker stack volumes (optional) | Pending — only if you abandon local CRDB/CH demos |

## What we never clear

- `deploy/.env.staging`, vault secrets, SSH keys  
- Staging droplet volumes (`v1_state`, `v2_state`, `twin_state`)  
- Git history  
- Source under `crates/`, `app-static/`, migrations  

## Regenerable (safe)

- `**/target/` (Cargo)  
- Docker build cache  
- `**/__pycache__/`  
- Dangling docker images  

## Tranche 1 results

### Docker build cache
- Ran: `docker builder prune -af`
- Reclaimed: **~21.67 GB**
- Left: running containers, images still in use, **all volumes**

### `vertical-1/` (work clone)
- Removed: `vertical-1/target/` (**580 MB**)
- Remaining source tree: **~516 KB**
- Kept: crates, proto, migrations, docker-compose, Cargo files

## Next session

Say **“next folder”** → clean **`vertical-2/`** (`target/` + any stale artifacts) and stop.

## Handoff

Full next-session handoff: `starting-out-documents/Session Handoff_ Context Transfer 2026-08-03.md` (also `SESSION_HANDOFF_2026-08-03.md`).

## Tranche 2 results (vertical-2)

- Removed: `vertical-2/target/` (~233416 KB before in work clone)
- Remaining source under vertical-2: source only (crates, Cargo, docs)
- Status: **Done 2026-08-03**
- Next folder: `vertical-3/`


## Tranche 3 results (vertical-3)

- **No `target/` present** in work clone (already clean)
- Folder size: **~624 KB** source only
- Freed this tranche: **~0 MB**
- Next: `vertical-security/`


## Tranche 4 results (vertical-security)

- No large `target/` in work clone (or removed if present)
- **Kept secrets/** (dev_secrets, examples) — never deleted vault files
- Status: **Done 2026-08-03**
- Next folder: `deploy/`
