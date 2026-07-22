# Vertical Security — Credential Egress Proxy

Centaur-inspired **outbound** credential injection for AI Manager.

Untrusted workers (and future agents) never hold long-lived API tokens. They call external APIs only through this proxy, which:

1. Looks up `X-AI-Manager-Tool` → tool registry entry  
2. Checks the target host is allowlisted for that tool  
3. Injects the secret from a vault/file backend as `Authorization` (or configured header)  
4. Forwards the request and returns the response  
5. Audit-logs `tool`, `host`, `status` (never secret values)  
6. Optionally redacts known secret substrings from response bodies  

**Inbound webhooks stay in-process (HMAC).** Only outbound API calls go through the proxy.

Fail-closed: unknown tool, unknown host, or missing secret → denied (no silent env fallback).

---

## Layout

```
vertical-security/
├── config/tool_registry.yaml      # tool → hosts + secret ref
├── secrets/dev_secrets.example.json
├── secrets/dev_secrets.json       # gitignored real secrets
├── src/                           # egress-proxy library + binary
├── docker-compose.yml
└── Dockerfile
```

---

## Quick start (local)

```bash
cd vertical-security

# Copy example secrets and edit
cp secrets/dev_secrets.example.json secrets/dev_secrets.json

# Unit + integration tests (mock upstream; no network to real GitHub)
cargo test

# Run proxy
cargo run -- \
  --bind 0.0.0.0:18090 \
  --registry config/tool_registry.yaml \
  --secrets secrets/dev_secrets.json
```

### Call through the proxy

Preferred path form (host after `/proxy/`):

```bash
# Health
curl -sf http://127.0.0.1:18090/healthz

# Allowlisted GitHub API (injects GITHUB_TOKEN; client sends NO Authorization)
curl -sS "http://127.0.0.1:18090/proxy/api.github.com/user" \
  -H "X-AI-Manager-Tool: github_api"

# Absolute URL form (also supported)
curl -sS "http://127.0.0.1:18090/https://api.github.com/user" \
  -H "X-AI-Manager-Tool: github_api"
```

### Denied host (TC-S03)

```bash
curl -si "http://127.0.0.1:18090/proxy/evil.example.com/x" \
  -H "X-AI-Manager-Tool: github_api"
# → 403 host not allowlisted for tool
```

---

## Docker Compose

```bash
cd vertical-security
cp secrets/dev_secrets.example.json secrets/dev_secrets.json
# edit secrets/dev_secrets.json with real tokens if you want live calls

docker compose up --build -d
curl -sf http://127.0.0.1:18090/healthz
```

---

## Configuration

| Env / flag | Default | Meaning |
|------------|---------|---------|
| `EGRESS_BIND` / `--bind` | `0.0.0.0:18090` | Listen address |
| `TOOL_REGISTRY` / `--registry` | `config/tool_registry.yaml` | Tool allowlist |
| `SECRETS_FILE` / `--secrets` | `secrets/dev_secrets.json` | JSON name→value map |
| `EGRESS_NO_REDACT` / `--no-redact` | false | Disable body redaction |
| `RUST_LOG` | `info` | tracing filter |

### Tool registry schema

```yaml
tools:
  github_api:
    hosts: ["api.github.com"]
    secret: GITHUB_TOKEN
    header: Authorization
    prefix: "Bearer "
```

### Secrets file

```json
{
  "GITHUB_TOKEN": "ghp_...",
  "SLACK_BOT_TOKEN": "xoxb-...",
  "WEBHOOK_SECRET_ten_demo": "whsec_demo"
}
```

Keys named `WEBHOOK_SECRET_<tenant_id>` can be overlaid into Vertical 1 tenant config (see V1 `TenantSecrets::from_file`).

---

## Test matrix (automated)

| ID | Scenario | How |
|----|----------|-----|
| **TC-S01** | Secrets load from file | `cargo test tc_s01` |
| **TC-S02** | Allowlisted host gets injected credential | `cargo test tc_s02` (mock upstream) |
| **TC-S03** | Non-allowlisted host → 403 | `cargo test tc_s03` |

### Manual steps (live proxy)

1. `cp secrets/dev_secrets.example.json secrets/dev_secrets.json` and set a real `GITHUB_TOKEN` if desired.  
2. `cargo run` (or `docker compose up --build`).  
3. `curl -sf http://127.0.0.1:18090/healthz` → `ok`.  
4. **TC-S02 live:**  
   `curl -sS "http://127.0.0.1:18090/proxy/api.github.com/user" -H "X-AI-Manager-Tool: github_api"`  
   Upstream receives `Authorization: Bearer <token>`; process env of the client need not contain the token.  
5. **TC-S03 live:**  
   `curl -si "http://127.0.0.1:18090/proxy/evil.example.com/" -H "X-AI-Manager-Tool: github_api"` → **403**.  
6. Confirm proxy logs show `egress audit` with `tool`, `host`, `status` and **no** token values.  
7. Stop proxy: clients with `EGRESS_ENFORCE=true` must fail closed (see Vertical 1 `EgressClient`).

---

## Vertical 1 client integration

In `vertical-1`, set:

```bash
export EGRESS_PROXY_URL=http://127.0.0.1:18090
export EGRESS_ENFORCE=true   # fail closed if proxy unset/unreachable — no env secret fallback
export SECRETS_FILE=../vertical-security/secrets/dev_secrets.json  # optional tenant webhook overlay
```

Use `telemetry_core::egress::EgressClient` for outbound HTTP.  
Smoke: `vertical-1/scripts/egress_smoke.sh` (curls proxy if up; does not start it).

---

## What this is not

- Not full Centaur K8s sandboxes / iron-proxy product  
- Not for inbound webhook HMAC (stays in V1 process)  
- Not a substitute for network policy that forces all egress through this proxy in production
