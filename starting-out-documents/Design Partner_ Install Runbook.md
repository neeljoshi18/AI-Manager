# Design Partner — Install Runbook (founder-operated)

**Goal:** Map **≥2 humans**, prove rare status digests, and hand the champion a working product loop — not a tour of half-finished surfaces.  
**Staging reference:** https://status.neel.world/app/  
**Time:** ~45–90 minutes if GitHub App + Slack bot already exist.

**Airtight done means:** both people get correct digests when work changes; Graph shows both; empty windows do **not** DM; same open PR does **not** re-DM every 30 minutes.

**Founder ops note:** Some campus Wi‑Fi networks block **SSH** to the staging droplet. Use a mobile hotspot only when a deploy is required. Day-to-day product checks use HTTPS only. Prefer **GitHub Actions** `Deploy staging` after one-time `STAGING_*` secrets (see `deploy/scripts/setup_ssh_via_https_port.md`).

**Cadence defaults (product):** status lookback **24h** (`STATUS_WINDOW_SECS=86400`); notify interval **30m** but **Notify Policy v1** (change-only + max 1 DM/person/UTC day) keeps Slack rare.

---

## 0. Preconditions

| Check | How |
|-------|-----|
| Stack healthy | `/app/` → **Connections**: V1 · V2 · V3 · egress green |
| Graph not mystery-empty | Connections shows **Graph: filled** (nodes &gt; 0) or “empty — re-projecting” (not silent 0/0) |
| Secrets | Slack bot token **only** in egress vault (`vertical-security/secrets/…`). Never on `twin-api` env (ADR-012) |
| Notify Policy v1 | Settings / `/metrics`: `notify_policy: v1_change_only_daily_cap` |
| Tenant | Default pilot tenant **`ten_github`** — this is *our* internal workspace id on staging (not a GitHub setting). Partners do not invent it; founder uses `ten_github` until multi-tenant self-serve exists. |

**Do not start** Linear / local model training until multi-person digests are proven (ADR-016).

---

## 1. GitHub (continuous ingest)

1. GitHub App webhook →  
   `https://<host>/v1/tenants/ten_github/webhooks/github`  
   (staging: `https://status.neel.world/v1/tenants/ten_github/webhooks/github`)
2. HMAC secret in vault / V1 secrets file (`WEBHOOK_SECRET_ten_github`).
3. Install App on the partner org/repos that should feed status.
4. Confirm **Connections**: last event age moves after a real PR open/sync.
5. Bridge projects V1 → V2 and upserts person twins for **mapped** actors only (bridge **never** Slack-DMs).

---

## 2. Slack (egress only)

1. Bot token in egress vault only.
2. Bot can open DMs to the two humans (app installed in workspace).
3. Optional team channel id for later publish (Approve path).
4. Confirm egress health on Connections.  
   **Never** put `SLACK_BOT_TOKEN` on twin-api.

---

## 3. Map two people (Team)

In **Team** (or env `SLACK_USER_MAP` on bridge):

For **each** human, save:

| Field | Example |
|-------|---------|
| Subject id | `gu_…` from first ingest, or map via GitHub login after first event |
| Display name | `Alex` |
| Slack user ID | `U0…` (from Slack profile) |
| Provider aliases | GitHub login + numeric id (`alice, 12345678`) |
| Channel (optional) | `C0…` |

**Done when:** Team shows `multi_person_ready: true` (API) / UI ready pill for ≥2 Slack-mapped people.

Bridge merges Team map every ~60s. Unmapped humans do **not** get default-Slack spam when a map exists.

---

## 4. First digests (prove the loop)

1. Each mapped human has open, non-empty work (PR/issue/**commit/push** in last **24h**, or an open PR) under ACL groups the graph can see. Unmapped or zero-edge people correctly show empty digests.
2. Optional ops (staging): `POST …/graph/ensure_users` then `POST …/team/prune` so multi-identity ACL neighborhoods compile.
3. **Team → Compile all digests** (policy-respecting) **or** wait for the scheduler window **or** **Send test status DM** once per person (force path for demo only). Compile response includes `with_items`, `item_kinds`, `empty_reason` per person.
4. **Today → Team digests** shows last draft status per person (empty = no DM).
5. In Slack DM, product language:

| Action | Meaning |
|--------|---------|
| **Approve** | Accurate — share / publish path |
| **Edit** | Fix the words |
| **Don't send** | Kill this draft (no channel post) |

6. In **My status**: items list shows summaries + **evidence** refs; empty window shows banner and **no** DM.
7. Open **Graph**: both **real** people appear; demo alice/bob seed is **hidden by default** (uncheck “Hide demo” or `include_demo=true` only for intent-demo UI proof). PR/issue/intent nodes present when work exists.

### Anti-spam check (must pass)

| Signal | Healthy |
|--------|---------|
| `/metrics` `twin_dms_suppressed_total` | Rising while same PR story stays open |
| `twin_dms_sent_total` | Low vs compiles (staging example: hundreds of suppressions, few DMs) |
| Same open PR for hours | **Not** a DM every 30 minutes |

---

## 5. Operator smoke (10 minutes)

```text
GET /healthz                          → twin-api ok
GET /metrics                          → notify_policy + suppress ≫ sent
GET /v3/demo/status                   → v1/v2/egress; graph_status ok|empty|v2_down
GET /v3/tenants/ten_github/team       → multi_person_ready true + last_digest per member
POST /v3/tenants/ten_github/graph/ensure_users  → 200 (membership for neighborhoods)
POST /v3/tenants/ten_github/team/compile  → with_items ≥1 when live work in lookback
GET /v3/tenants/ten_github/graph      → real people; demo_hidden ≥0; no alice/bob by default
GET /v3/tenants/ten_github/pulse      → live conflicts/intents (demo_* separate)
```

Embedded staging: twin state file (`TWIN_EMBEDDED_STATE_PATH`) + `SLACK_USER_MAP` seed keep multi-person maps across twin-api restarts.


If Graph is empty after a V2 restart: wait &lt;2 minutes for bridge **recovery mode** (clears seen, burst re-project). Autoheal restarts wedged V2.

---

## 6. Hand off to learning window

When multi-person digests are real:

1. Point champion at `Design Partner_ One-Pager.md` (what / not / success).
2. Run `Design Partner_ Learning Window Playbook.md` (10–14 days).
3. Kill criteria: 100% Don't-send with angry users, or zero digests after real work, or spam returns → pause outreach and fix before expanding connectors.

---

## 7. What we never do in install

- Silent private 1:1 Slack wiretap (ADR-015)
- Tokens in twin worker env (ADR-012)
- Promise search-everything / on-prem custom GPT of the company day one (ADR-006 / ADR-016)
- Ship a second half-wired connector instead of two working digests

---

## Related

- Handoff: `Session Handoff_ Context Transfer 2026-07-27.md`
- Plan: `plans/2026-07-27_confidence-airtight-pilot.md`
- Deploy: `deploy/README.md` · compose `deploy/docker-compose.app.yml`
- Notify Policy: `vertical-3/crates/twin-core/src/notify_policy.rs`
