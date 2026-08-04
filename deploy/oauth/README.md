# OAuth / App install scaffolding (M5)

Code and manifests live here so staging install does not depend on tribal knowledge.
**Do not commit client secrets.** Human supplies credentials when ready.

## Slack (OAuth install)

| Item | Value |
|------|--------|
| Manifest | `slack-app-manifest.json` |
| Token storage | **Only** `vertical-security/secrets/dev_secrets.json` key `SLACK_BOT_TOKEN` (ADR-012) |
| Scopes | `chat:write`, `im:write` (outbound status DMs); events later for channel ingest (M6) |
| Install flow | UI button → Slack OAuth → store bot token in egress vault → twin uses `USE_EGRESS_SLACK` |

### Human steps when secrets are ready

1. Create Slack app from manifest at [api.slack.com/apps](https://api.slack.com/apps).
2. Set Redirect URL to `https://$DOMAIN/v3/oauth/slack/callback` (wired: exchanges code → writes vault).
3. Put `SLACK_CLIENT_ID` + `SLACK_CLIENT_SECRET` in `deploy/.env.staging` (not git).
4. Product UI → **Connections → Connect Slack** (or Cockpit → Connect Slack/GH).
5. Callback writes `SLACK_BOT_TOKEN` into `OAUTH_VAULT_PATH` (`/secrets/dev_secrets.json` on staging).
6. **Restart egress** so it reloads the vault (or full deploy).
7. Never put `SLACK_BOT_TOKEN` on twin-api env (ADR-012).

### Manual path (always works)

Paste bot token into `vertical-security/secrets/dev_secrets.json` as `SLACK_BOT_TOKEN` and restart egress.

### Blocked on human

- Slack Client ID / Client Secret (for OAuth button)
- Public HTTPS redirect URL (staging domain)

## GitHub App

| Item | Value |
|------|--------|
| Manifest | `github-app-manifest.yml` |
| Webhook URL | `https://$DOMAIN/v1/tenants/{tenant_id}/webhooks/github` |
| Prefer App | Over personal PAT + ngrok for real tenants |
| HMAC secret | Tenant config / vault (`WEBHOOK_SECRET_*`) — not twin env |

### Human steps when secrets are ready

1. Create GitHub App (or use manifest flow) with webhook URL above.
2. Subscribe to PR / issues / push as needed.
3. Install on target org/repos; store webhook secret for tenant `ten_github` (or product tenant).
4. Retire ngrok for that tenant.

### Blocked on human

- GitHub App ID / private key / webhook secret
- Host choice + public DNS for webhook delivery

## Product UI

Connections page shows disabled **Install GitHub App** / **Connect Slack OAuth** until these credentials exist.
Manual path remains: vault token + webhook → V1.
