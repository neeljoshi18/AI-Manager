# AI Manager

Private monorepo for the Autonomous AI Manager platform (engineering context layer; anti-Glean strip-to-win).

## Layout

| Path | Purpose |
|------|---------|
| `starting-out-documents/` | Strategy, ADRs, handoff, **human demo script**, M4 plan |
| `vertical-1/` | Telemetry ingestion, canonical events, ACL |
| `vertical-2/` | Organizational Context Graph (ACL-safe multi-hop) |
| `vertical-3/` | Status twins, ledgers, veto-first Slack + **demo console** |
| `vertical-security/` | Credential **egress proxy** (outbound secret inject) |
| `scripts/platform_sew.sh` | Cross-vertical sew battery (TC-P01…) |

**Product completeness** ≈ verticals **sewn** (V1→V2→V3→egress Slack), not V1 alone.

## Milestone map

| M | Meaning | Status |
|---|--------|--------|
| M1–M3 | Engines V1 / V2 / V3 | Done (code + unit batteries) |
| **M4** | Live sew + **demo console** + real Slack | Done enough |
| **M5** | Staging single-tenant deploy + product UI | **In progress** |
| M6 | Design-partner weekly use | Later |
| M7 | Self-serve deployed product | “Finished” for teams |

## 2-minute demo (leads / Reddit / X)

```bash
./scripts/dev_up.sh
# Product UI (redesigned shell):
open http://127.0.0.1:18083/app/
# Lab console:
open http://127.0.0.1:18083/demo/
```

**Docker multi-service (staging-shaped):**

```bash
docker compose -f deploy/docker-compose.app.yml up -d --build
open http://127.0.0.1:18083/app/
```

Wake after sleep: [plans/2026-07-23_wake-laptop-runbook.md](./plans/2026-07-23_wake-laptop-runbook.md).  
Deploy: [deploy/README.md](./deploy/README.md).

Full script: [Human Demo Script](./starting-out-documents/Human%20Demo%20Script.md)

## Platform sew

```bash
# Embedded (V3 only ok):
./scripts/platform_sew.sh

# Live (requires V1 :18080, V2 :18082, V3 :18083):
SEW_MODE=live ./scripts/platform_sew.sh
```

## Key documents

- [Session Handoff](./starting-out-documents/Session%20Handoff_%20AI%20Manager%20State.md)
- [**Product Roadmap — Intent → Twins**](./starting-out-documents/Product%20Roadmap_%20Intent%20Capture%20to%20Digital%20Twins.md) ← **what we build next**
- [**Plans/**](./plans/) ← dated plan snapshots (start here after each planning session)
- [Interaction Log](./starting-out-documents/Interaction%20Log_%20Product%20Decisions.md) ← decision history
- [Human Demo Script](./starting-out-documents/Human%20Demo%20Script.md)
- [Architecture Decision Log](./starting-out-documents/Architecture%20Decision%20Log_%20Pivotal%20Choices.md)
- V1 / V2 / V3 TAS under `starting-out-documents/` and `vertical-*/`

## Vertical quick starts

**V1:** `cd vertical-1 && SKIP_AUTH=true cargo run -p telemetry-ingestion` → `:18080`  
**V2:** `cd vertical-2 && RUNTIME_MODE=embedded cargo run -p graph-api` → `:18082`  
**V3:** `cd vertical-3 && RUNTIME_MODE=embedded cargo run -p twin-api` → `:18083` + `/demo/`  
**Egress:** `cd vertical-security && cargo run` → `:18090`  

## Real Slack (M4)

Bot token lives **only** in `vertical-security/secrets/dev_secrets.json` (`SLACK_BOT_TOKEN`).  
Twin process: `USE_EGRESS_SLACK=true EGRESS_PROXY_URL=http://127.0.0.1:18090` — **never** set `SLACK_BOT_TOKEN` on twin.
