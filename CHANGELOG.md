# Changelog

Notable changes of the workspace. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions are git tags; all crates share the workspace version.

## [Unreleased]

### Added

- Commodities: an item type flagged `commodity` takes no bids, needs a buyout price and can be bought in any quantity up to what is left. The seller gets back the matching share of the deposit; the rest is lost on cancellation or expiry. Other items are auctioned and bought whole. ([ADR 0012](docs/adr/0012-commodities-bought-in-part.md))
- `game` catalog: optional `commodity` key. `game` interface: quantity input to buy part of a commodity.
- Rustdoc on the public items of the Agora, `event-sourcing`, `ledger`, game and tcg crates. The rustdoc of `game-dto` and `tcg-dto` is the HTTP reference.
- `docs/functional.md`, `docs/architecture.md`, `docs/roadmap.md`, architecture decision records in `docs/adr/`, and this changelog.

### Changed

- **Breaking**: `AuctionApi::buyout` and `GameApi::buyout` take a quantity, and the `game` buyout request carries one.
- New `ledger` crate shared by `game` and `tcg`: `Account`, its deciders, the treasury, `LedgerApi`, `LedgerService` and `AgoraBridge`. `Player` and `Collector` become aliases of `Account`; ledger errors move to `LedgerError`. ([ADR 0007](docs/adr/0007-event-sourced-shared-ledger.md))
- Documentation in English, split into functional and technical documents, current state and target.

### Removed

- `docs/PRD.md`. Its content moved to the documents above.

## [0.2.0] - 2026-10-18

### Added

- Trading card game demo `tcg`: collectors hold dust and interchangeable card copies, a card's base value comes from its rarity, a listing always holds one card. Server on port 8081, interface on port 3001. ([ADR 0011](docs/adr/0011-one-card-per-listing.md))
- Event-sourced game ledger: player gold and items and the treasury are event streams, each movement records the `OpId` of the Agora call so a replayed call records nothing, and read models are projections caught up on every read. ([ADR 0007](docs/adr/0007-event-sourced-shared-ledger.md), [ADR 0008](docs/adr/0008-read-models-caught-up-on-read.md))
- `game-server` settings in `game.toml` (or `GAME_CONFIG`): listen address, settlement delay, log levels per target, auction houses and item catalog, with validation. `BIND_ADDR` and `RUST_LOG` still override the file. ([ADR 0010](docs/adr/0010-toml-settings.md))
- Starting players `alice`, `bob` and `carol` with random gold and items from the catalog.
- `game` interface: prices typed per unit or per stack, stack totals in the listings table.

### Changed

- **Breaking**: `PlayerRepository` and `Treasury` replaced by `EventStore`, `Projection`, `PlayerView` and `TreasuryView`.
- Event sourcing moved out of `game-core` and `game-spi` into the domain-independent `event-sourcing` crate.
- The catalog is an inline `catalog` array in `game.toml`.
- The interface refreshes every second instead of every 2 seconds.

### Fixed

- Replayed effects were never recognized by the ledger, because every port call got a fresh `OpId`. Effects of a saved listing change now get a stable UUID v5 `OpId`, compensations one derived from the call they undo. ([ADR 0006](docs/adr/0006-stable-operation-ids.md))

## [0.1.0] - 2026-10-15

### Added

- Auction core with a hexagonal architecture: `agora-domain`, `agora-api` (`AuctionApi`), `agora-spi` (`Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`) and `agora-core` (`AuctionService`, in-memory adapters behind the `memory` feature). ([ADR 0001](docs/adr/0001-hexagonal-auction-core.md))
- Listings with escrow, deposit, bids, buyout, cancellation, settlement of expired listings and search; optimistic locking with retries. ([ADR 0004](docs/adr/0004-optimistic-locking.md), [ADR 0005](docs/adr/0005-escrow-and-deposit.md))
- Demo game split into clean architecture crates (`game-domain`, `game-api`, `game-spi`, `game-core`, `game-server`, `game-dto`, `game-ui`), HTTP API on port 8080, Yew interface on port 3000. The base value of a listed item is read from the catalog by the game core. ([ADR 0003](docs/adr/0003-trusted-game-server.md))
- Structured logging with `tracing`: business events, rejected requests, settlement and every port call with its `OpId`.
- Interface: deposit shown in the sell form.
