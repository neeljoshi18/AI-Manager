# AI Manager

Private monorepo for the Autonomous AI Manager platform (engineering context layer; anti-Glean strip-to-win).

## Layout

| Path | Purpose |
|------|---------|
| `starting-out-documents/` | Ground-truth strategy + architecture + **decision log** |
| `vertical-1/` | Telemetry ingestion, canonical events, ACL, ClickHouse/Redpanda |
| `vertical-2/` | Organizational Context Graph (spec first; implement next) |
| `vertical-security/` | Centaur-inspired credential **egress proxy** (outbound secret inject) |

Future verticals: `vertical-3/`, … — **one folder per vertical**, no nested coupling.

## Key documents

- [Architecture Decision Log — Pivotal Choices](./starting-out-documents/Architecture%20Decision%20Log_%20Pivotal%20Choices.md)
- [Vertical 1 Technical Architecture Spec](./starting-out-documents/Technical%20Architecture%20Specification_%20Vertical%201.md)
- [Vertical 2 Technical Architecture Spec](./vertical-2/Technical%20Architecture%20Specification_%20Vertical%202.md)
- Competitive analysis docs under `starting-out-documents/`

## Vertical 1 quick start

```bash
cd vertical-1
docker compose up -d          # optional production backends
SKIP_AUTH=true cargo run -p telemetry-ingestion   # embedded default unless .env production
```

See `vertical-1/README.md`.

## Vertical 2

See `vertical-2/README.md`.

## Credential egress (vertical-security)

Outbound API calls inject secrets via a small Rust reverse proxy so workers never hold long-lived tokens.

```bash
cd vertical-security
cp secrets/dev_secrets.example.json secrets/dev_secrets.json
cargo test && cargo run   # :18090
```

See [vertical-security/README.md](./vertical-security/README.md).
