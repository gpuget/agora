# 0001. Hexagonal auction core, independent of any game

- Status: Accepted
- Date: 2026-10-14

## Context

Agora is meant to be plugged into games it does not know. Each game owns its players, their money and their items, and decides what happens to the money taken by the auction houses.

## Decision

Split Agora into ports and adapters:

- `agora-domain`: types and pure rules.
- `agora-api`: inbound port `AuctionApi`, the use cases.
- `agora-spi`: outbound ports `Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`.
- `agora-core`: `AuctionService`, which implements `AuctionApi` with the outbound ports only.

Ports are async traits whose methods return `impl Future + Send`. `AuctionService` is generic over its ports, with no `dyn`.

The demo games follow the same layering (domain, api, spi, core, server, ui), and see Agora as an external system behind the `AuctionApi` outbound port.

## Consequences

- The Agora crates depend neither on tokio nor on axum, and can be tested with in-memory adapters and a manual clock.
- A game integrates Agora by implementing `Inventory`, `Wallet` and `MoneySink`. The demo games share one implementation, `AgoraBridge` ([0007](0007-event-sourced-shared-ledger.md)).
- Generic services make the wiring types long (see `type Agora = AuctionService<...>` in each server), in exchange for static dispatch.
