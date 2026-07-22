# Wake-from-sleep runbook (local demo)

**Date:** 2026-07-23  

Laptop sleep usually **kills** Rust services and often **ngrok**. Docker may keep Cockroach/Redis if you use compose.

## One-shot restart

```bash
cd /path/to/ai-manager
./scripts/dev_down.sh   # optional clean
./scripts/dev_up.sh
```

Open **http://127.0.0.1:18083/app/**

## Optional pieces

| Piece | Command |
|-------|---------|
| Real Slack token | `vertical-security/secrets/dev_secrets.json` must exist before `dev_up` |
| ngrok (GitHub webhooks) | `ngrok http 18080` — update GitHub webhook URL if host changes |
| GitHub bridge only | `TENANT_ID=ten_github python3 scripts/github_live_bridge.py` |

## Health check

```bash
curl -s http://127.0.0.1:18083/v3/demo/status
```

Expect `"v1":true,"v2":true,"v3":true`. Egress true only if proxy + secrets up.

## Logs

`/tmp/ai-manager-dev/*.log` when using `dev_up.sh`.
