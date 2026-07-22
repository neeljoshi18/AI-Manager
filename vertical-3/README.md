# Vertical 3 — Status Twins, Ledgers & Veto-First Delivery

**Status:** Specification complete. Implementation not started (next session).

## What it is

Compiles **status ledgers** from Vertical 2’s ACL-safe graph, delivers them **privately first** (Slack DM), respects **confidence tiers** and **human veto**, then publishes to the team channel. This is the meeting-killing product layer.

## Spec (ground truth)

**[Technical Architecture Specification_ Vertical 3.md](./Technical%20Architecture%20Specification_%20Vertical%203.md)**

Stick to that document. If ambiguous, prefer §1.2 invariants.

## Planned layout

```
vertical-3/
├── Technical Architecture Specification_ Vertical 3.md
├── README.md
├── Cargo.toml                 # (implementation phase)
├── crates/
│   ├── twin-core/
│   ├── twin-compiler/
│   ├── twin-delivery/
│   ├── twin-api/              # :18083
│   └── twin-verify/           # TC-T01…T10
├── migrations/cockroach/
└── scripts/
    ├── smoke_v3.sh
    └── sew_e2e.sh
```

## Ports

| Service | Port |
|---------|------|
| V1 | 18080 |
| V2 | 18082 |
| **V3 twin-api** | **18083** |
| Egress proxy | 18090 |

## Dependencies

- **V2 graph-api** — neighborhood, blockers, state (ACL QueryContext)
- **vertical-security egress-proxy** — Slack writes only (no bot token in twin env)
- Shared Cockroach → database `status_twins`
- Optional Redis for locks / veto timers

## Product stance

- Not Glean (no search index)
- Not Buzz (no workspace/Git forge)
- Not Centaur agent OS (egress inject only)
- No individual productivity rankings

## Implementation order

See TAS §14. Summary: `twin-core` → migrations → compiler → delivery → API → verify → sew script.

## Related docs

- `../starting-out-documents/Session Handoff_ AI Manager State.md`
- `../starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` (ADR-013)
- `../vertical-2/Technical Architecture Specification_ Vertical 2.md`
- `../vertical-security/README.md`
