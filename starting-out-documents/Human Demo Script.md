# Human Demo Script — AI Manager (Sew & Show / M4)

**Audience:** You (founder), design partners, Reddit/X leads  
**Time:** ~2 minutes with demo console; ~5 minutes with real Slack  

---

## What you are demoing

> AI Manager kills standup theater. It compiles a **status ledger** from engineering activity (PRs/tickets), delivers it **privately first** in Slack, and only posts to the team channel after **veto / edit / consent**.  
> No enterprise search crawl. No agent sandbox OS. ACL never bypassed.

---

## A. Fastest path (demo console, no Slack)

1. Terminal:
   ```bash
   cd vertical-3
   RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 cargo run -p twin-api
   ```
2. Browser: **http://127.0.0.1:18083/demo/**
3. Click **Simulate PR → Ledger → Draft**
4. Show:
   - Confidence tier (Medium for open PR)
   - Evidence refs (`event:…`, `edge:…`)
   - Draft status `pending`
5. Click **Veto** or **Publish** / **Silence timeout**
6. Pitch line: *“Private first. Structure first. No productivity rankings.”*

---

## B. Platform sew (terminal proof V1/V2/V3)

```bash
# With only V3 up:
./scripts/platform_sew.sh

# With all three APIs up:
SEW_MODE=live ./scripts/platform_sew.sh
```

Expect `PLATFORM SEW OK` and TC-P\* PASS/SKIP rows.

---

## C. Real Slack DM (requires your bot token)

1. Create Slack app + bot token (see plan §3).  
2. Put token **only** in `vertical-security/secrets/dev_secrets.json` as `SLACK_BOT_TOKEN`.  
3. Run egress:
   ```bash
   cd vertical-security
   cargo run -- --bind 0.0.0.0:18090 --registry config/tool_registry.yaml --secrets secrets/dev_secrets.json
   ```
4. Run twin-api with egress Slack:
   ```bash
   cd vertical-3
   RUNTIME_MODE=embedded SHADOW_MODE_DAYS=0 \
     USE_EGRESS_SLACK=true \
     EGRESS_PROXY_URL=http://127.0.0.1:18090 \
     EGRESS_ENFORCE=true \
     cargo run -p twin-api
   ```
5. In demo console, set **Slack user ID** to your `U…` member ID → Simulate.  
6. **You should receive a DM** from the bot with the status draft.

Never put `SLACK_BOT_TOKEN` in twin process env.

---

## D. Screenshot checklist (for posts)

- [ ] Demo console health pills  
- [ ] Ledger items with evidence  
- [ ] Draft status after veto/publish  
- [ ] (Optional) Phone screenshot of Slack DM  

---

## E. One-liners for Reddit / X

- “We’re building the anti-Glean for eng status: no search index, ACL-safe graph, Slack veto-first standups.”  
- “Success metric is meetings deleted, not chat engagement.”  
- “Agents shouldn’t hold Slack tokens. Egress inject only.”  

---

## F. Milestone map (remember)

| M | Meaning |
|---|--------|
| M3 | Engines V1–V3 (you are past this) |
| **M4** | Live sew + **visible demo** + real Slack (this phase) |
| M5 | Staging deploy on a host |
| M6 | Design partner weekly use |
| M7 | Self-serve deployed product |
