# Vertical 2 — Organizational Context Graph

**Status:** Architecture specified; implementation not started.

This folder is the isolated home for Vertical 2 (sibling of `vertical-1/`).

## What it is

Vertical 2 projects Vertical 1 telemetry into a **temporal, ACL-safe context graph** (entities, lineage, blockers, multi-hop paths) so later verticals (digital twins, status orchestration) do not rely on flat search chunks.

## Spec (ground truth for V2 build)

→ **[Technical Architecture Specification_ Vertical 2.md](./Technical%20Architecture%20Specification_%20Vertical%202.md)**

## Stack (decision)

| Layer | Choice |
|-------|--------|
| Runtime | Rust / Axum (same family as V1) |
| Input | V1 Redpanda topics only |
| Graph store | **CockroachDB** database `context_graph` (property graph tables) |
| ACL | Live groups from V1 + denormalized allow-lists on nodes/edges |
| Not used | Neo4j/Memgraph (day one), vectors, full-text, ClickHouse-as-graph |

Rationale and alternatives:  
`../starting-out-documents/Architecture Decision Log_ Pivotal Choices.md` → **ADR-007**

## Layout (when implemented)

```
vertical-2/
├── Technical Architecture Specification_ Vertical 2.md
├── README.md
├── Cargo.toml
├── crates/graph-core|graph-projector|graph-api|graph-verify
├── migrations/cockroach/
└── scripts/
```

## Dependency rule

- **V2 may consume V1** (bus + read identity).
- **V1 must never depend on V2.**
- **No webhooks in V2.**

## Next step

Implement per the Technical Architecture Specification when product prioritizes Vertical 2 build.
