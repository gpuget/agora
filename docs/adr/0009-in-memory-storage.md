# 0009. In-memory storage for the prototype

- Status: Accepted
- Date: 2026-10-14

## Context

Agora is a prototype. Its storage sits behind ports (`AuctionRepository`, `EventStore`, `Projection`, `HouseRegistry`, `Catalog`).

## Decision

Only in-memory adapters exist, behind the `memory` feature of `agora-core`, `event-sourcing`, `game-core` and `tcg-core`. The servers use them.

## Consequences

- No database to install: `cargo run` is enough.
- All state is lost when a server restarts.
- Persistent storage is planned ([roadmap](../roadmap.md)) and will be new adapters of the same ports.
