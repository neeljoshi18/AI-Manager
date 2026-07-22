# GitHub → V1 local setup (real commits/PRs)

**Goal:** When you open a PR or push on GitHub, V1 accepts the webhook → V2 graph → V3 ledger → Slack.

You do **not** need a broad GitHub PAT for inbound webhooks. You need:
1. A **webhook secret** (you invent a random string)
2. A **public URL** to your laptop’s V1 (`:18080`) — usually **ngrok**
3. A **webhook** configured on a repo (or org)

---

## Steps for you

### 1. Install ngrok (if needed)

```bash
# macOS
brew install ngrok
# or download from https://ngrok.com
ngrok config add-authtoken <your ngrok token from dashboard>
```

### 2. Start the stack (if not already)

Leave these running:

| Service | Port |
|---------|------|
| V1 telemetry-ingestion | 18080 |
| V2 graph-api | 18082 |
| V3 twin-api | 18083 |
| egress-proxy | 18090 |

### 3. Expose V1

```bash
ngrok http 18080
```

Copy the HTTPS URL, e.g. `https://abc123.ngrok-free.app`

### 4. Choose a webhook secret

Invent a long random string, e.g.:

```text
whsec_ai_manager_local_CHANGE_ME_9f3a
```

### 5. Register the tenant on V1 (we can also do this for you)

```bash
curl -sS -X POST http://127.0.0.1:18080/v1/tenants \
  -H 'content-type: application/json' \
  -d '{
    "tenant_id": "ten_github",
    "github_webhook_secret": "whsec_ai_manager_local_CHANGE_ME_9f3a",
    "default_group_ids": ["grp_eng"]
  }'
```

### 6. Create the GitHub webhook

1. Open your test repo on GitHub → **Settings** → **Webhooks** → **Add webhook**  
2. **Payload URL:**  
   `https://YOUR-NGROK-HOST/v1/tenants/ten_github/webhooks/github`  
3. **Content type:** `application/json`  
4. **Secret:** same string as step 4  
5. **Events:** “Let me select…” → check **Pull requests** and **Pushes** (minimum)  
6. Active → **Add webhook**

### 7. What to send back in chat (paste here)

```text
GITHUB_WEBHOOK_SECRET=whsec_...
GITHUB_REPO=owner/repo
NGROK_URL=https://....ngrok-free.app
```

Optional (only if we need outbound GitHub API later — **not** required for webhooks):

```text
GITHUB_TOKEN=ghp_...   # fine-grained, read metadata only if possible
```

If you give a token, it goes **only** in `vertical-security/secrets/dev_secrets.json` (egress), never in twin env.

### 8. Trigger

Open a small PR or push a commit on that repo. We will verify:

- V1 `/events` shows the event  
- V2 neighborhood shows the PR node  
- V3 compile → Slack DM with human-readable status  

---

## Security

- Prefer a **dedicated test repo**  
- Webhook secret ≠ GitHub password  
- Rotate secrets if pasted into chat  
- ngrok URLs are temporary; update the webhook if ngrok restarts  
