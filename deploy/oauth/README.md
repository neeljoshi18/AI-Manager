# OAuth / App install scaffolding (M5)

Code and manifests live here so staging install does not depend on tribal knowledge.
**Do not commit client secrets.** Human supplies credentials when ready.

**Doctrine:** Slack (or Teams) = **delivery**. GitHub = **work signals**. Tokens only in the egress vault (ADR-012). Never put bot tokens on twin-api env.

---

## On a sales call (champion path)

White-glove sequence that should never look broken:

1. **Connections → Connect Slack**  
   Bot install OAuth → success page → return lands on Connections (`/app/?view=connections`).
2. **Paste Events Request URL** `https://$DOMAIN/v3/slack/events` in Slack app → Event Subscriptions  
   (bot events: `message.channels`, `message.groups`, `message.im`). Then **invite the bot** to the team channel for ambient claims.  
   **Digests still DM** mapped people if the bot is not in a channel.
3. **Install GitHub App** (work signals) → copy webhook URL into App settings if needed → pick org/repos.
4. **Map pod under Team** (Slack user ids / bulk import).
5. **Open Cockpit** (digests, pulse, tomorrow focus) / Graph after ~1 min for work to land.

After first Slack OAuth: **restart egress once** so it reloads the vault (only when vault write succeeded). No secrets are shown on success pages or in chat.

UI checklist on Connections turns green from `/v3/oauth/status` (and soft graph healthy from demo status). Use **I finished install — refresh status** after either Slack or GitHub.

Keep **`DELIVERY_ADAPTER=slack`** (default) unless the tenant explicitly wants Teams.

---

## Slack (OAuth install)

| Item | Value |
|------|--------|
| Manifest | `slack-app-manifest.json` |
| Token storage | **Only** `vertical-security/secrets/dev_secrets.json` key `SLACK_BOT_TOKEN` (ADR-012) |
| Scopes | `chat:write`, `im:write`, `im:history`, `users:read` (DMs) + `channels:history`, `channels:read`, `groups:history`, `groups:read` (opt-in channel intent) |
| Events | `message.im` (approve / don't send + DM free-text claims) · `message.channels` / `message.groups` (channel intent claims) |
| Install flow | UI button → Slack OAuth → store bot token in egress vault → twin uses `USE_EGRESS_SLACK` |

### Channel intent (opt-in team truth — not private wiretap)

- **Bot must be invited** to a public/private channel before channel messages are visible to the Events API.
- Twin-api classifies high-confidence phrases (`blocked on`, `working on`, `freeze`, `ready to ship`, …) into typed **intent claims** (preview ≤280 chars) — not a full chat archive.
- Stored as tenant_kv `slack_intent_claims` + observer kind `slack_intent`; surfaces on person profile / follow-through.
- **No silent 1:1 wiretap:** private human↔human DMs are never read. Only (1) bot DMs for digest approve/veto + explicit free-text claims, and (2) channels where the bot is a member.

### Human steps when secrets are ready

1. Create Slack app from manifest at [api.slack.com/apps](https://api.slack.com/apps).
2. Set Redirect URL to `https://$DOMAIN/v3/oauth/slack/callback` (wired: exchanges code → writes vault).
3. Put `SLACK_CLIENT_ID` + `SLACK_CLIENT_SECRET` in `deploy/.env.staging` (not git).
4. Product UI → **Connections → Connect Slack** (or Cockpit → Connect Slack/GH).
5. Callback writes `SLACK_BOT_TOKEN` into `OAUTH_VAULT_PATH` (`/secrets/dev_secrets.json` on staging). Never displays the token.
6. **Restart egress** so it reloads the vault (or full deploy) — only needed after a successful vault write.
7. Confirm Event Subscriptions Request URL is `https://$DOMAIN/v3/slack/events`. Invite bot to team channel for ambient claims; digests still DM without that once people are mapped. Re-run **Connect Slack** if the token predates channel-history scopes.
8. Never put `SLACK_BOT_TOKEN` on twin-api env (ADR-012).

### Manual path (always works)

Paste bot token into `vertical-security/secrets/dev_secrets.json` as `SLACK_BOT_TOKEN` and restart egress.

### Blocked on human

- Slack Client ID / Client Secret (for OAuth button)
- Public HTTPS redirect URL (staging domain)

---

## GitHub App

| Item | Value |
|------|--------|
| Manifest | `github-app-manifest.yml` |
| Webhook URL | `https://$DOMAIN/v1/tenants/{tenant_id}/webhooks/github` |
| Prefer App | Over personal PAT + ngrok for real tenants |
| HMAC secret | Tenant config / vault (`WEBHOOK_SECRET_*`) — not twin env |

### Human steps when secrets are ready

1. Create GitHub App (or use manifest flow) with webhook URL above (Connections always shows a copyable URL).
2. Subscribe to PR / issues / push as needed.
3. Install on target org/repos; store webhook secret for tenant `ten_github` (or product tenant).
4. Return to Connections → **I finished install — refresh status** → open Graph / Cockpit.
5. Retire ngrok for that tenant.

### Blocked on human

- GitHub App ID / private key / webhook secret
- Host choice + public DNS for webhook delivery

---

## Microsoft Teams (delivery adapter — secondary)

| Item | Value |
|------|--------|
| Manifest | `teams-app-manifest.json` |
| Token storage | **Only** vault key `TEAMS_BOT_TOKEN` (Bot Framework bearer) — ADR-012 |
| Messaging endpoint | `https://$DOMAIN/v3/teams/messages` |
| Map | Team member `teams_user_id` (AAD / Teams user id) on twin config |
| Actions | Adaptive Cards: **Approve · Edit · Don't send** (same as Slack) |
| Select adapter | `DELIVERY_ADAPTER=teams` + `USE_EGRESS_TEAMS=true` (**default remains Slack**) |

### Human steps when secrets are ready

1. Create Azure Bot + Teams app (use `teams-app-manifest.json` as a starting point; replace app ids).
2. Set Bot messaging endpoint to `https://$DOMAIN/v3/teams/messages`.
3. Put Bot Framework connector token in vault as `TEAMS_BOT_TOKEN` (never on twin-api env).
4. Set public env: `TEAMS_APP_ID`, optional `TEAMS_TENANT_ID` / `TEAMS_SERVICE_URL`.
5. Set `DELIVERY_ADAPTER=teams` and `USE_EGRESS_TEAMS=true` in compose / `.env.staging`.
6. Restart **egress** then **twin-api** so vault + adapter load.
7. Map members with `teams_user_id` (Team API / bulk paste path can extend later).

### Manual path (always works)

Paste connector token into `vertical-security/secrets/dev_secrets.json` as `TEAMS_BOT_TOKEN` and restart egress.

### Blocked on human

- Azure Bot App ID + password / connector token  
- Teams admin consent for the org  

**Do not break Slack:** leave `DELIVERY_ADAPTER=slack` (default) for existing pilots.

---

## Google / SSO (identity plane — later)

SSO grants seats + **champion vs member** roles only. It does **not** replace Connect Slack/Teams or GitHub. Ship with multi-tenant packaging.

---

## Product UI

Connections page: **Install GitHub App** / **Connect Slack** / **Connect Teams** / roles panel.  
Post-OAuth: `/app/?view=connections&connected=slack` auto-opens Connections + refresh.  
SSO shows as roadmap until multi-tenant packaging.  
Manual path remains: vault token + webhook → V1.

API: `GET /v3/oauth/status` returns `next_steps`, `install_checklist`, and Slack `scopes` (booleans only — no secret values).
