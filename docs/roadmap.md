# Agora: roadmap

What is planned or deliberately left out today. Nothing here is implemented. The current state is in [functional.md](functional.md) and [architecture.md](architecture.md). When an item below is decided, it gets an [ADR](adr/); when it is built, the current-state documents describe it.

## Planned

### Failure resilience: transactional outbox

Today, if a port fails in the middle of a use case, money or items can be lost or duplicated.

Target: the external effects of a listing change are written to the database in the same transaction as the change (`pending_operations`), then executed and replayed until they succeed. The ledger already ignores a replayed `OpId`, and every effect already gets a stable `OpId` ([ADR 0006](adr/0006-stable-operation-ids.md)), so a replay is safe on the ledger side.

Depends on persistent storage.

### Persistent storage

Postgres adapters for the Agora repository and the event stores, replacing the in-memory adapters of [ADR 0009](adr/0009-in-memory-storage.md).

## Out of scope for now

- **Agora events**: an `EventPublisher` port for events such as sale concluded or bid outbid.
- **Market features**: price history, filters on item attributes, text or category search.
- **Multi-tenant**: several games in one Agora instance.
- **Player authentication**: today the demo servers accept any player ([ADR 0003](adr/0003-trusted-game-server.md)).

## Known limits with no plan yet

Listed so they are not forgotten. Each needs a decision before work starts.

- The set of applied `OpId`s of a ledger stream is never purged.
- `execute` reloads the whole stream on every command, with no snapshot.
- Item attributes do not survive the ledger, which counts items per item type.
- Settlement has no end-to-end test through the HTTP API.
- `ListingStatus::Sold` records only the last buyer of a commodity sold in parts.
