# Agora: architecture

How Agora and its demo games are built. This document describes the current state. Business rules are in [functional.md](functional.md), the decisions behind this design in [adr/](adr/), and the target in [roadmap.md](roadmap.md). The API of each crate is documented in its rustdoc: `cargo doc --no-deps --workspace --open`.

## 1. Overview

The workspace has three parts:

- **Agora**, the auction core (`agora/`), independent of any game. Hexagonal: the domain and the use cases sit in the middle, and the outside world is reached through ports. See [ADR 0001](adr/0001-hexagonal-auction-core.md).
- **Shared building blocks**: `event-sourcing`, a domain-independent event sourcing library, and `ledger`, the event-sourced ledger of players' money and items used by both demo games.
- **Two demo games**, `game/` and `tcg/`, each with the same layers: domain, ports, core, HTTP server, web interface.

```
                 +-----------+        +-----------+
   browser ----> |  game-ui  |  HTTP  |game-server| ---- wiring, HTTP adapter, clock, settings
                 +-----------+ -----> +-----------+
                                            |
                                      +-----------+
                                      | game-core |  GameService (GameApi)
                                      +-----------+
                                       |         |
                     AuctionApi (port) |         | PlayerView, TreasuryView (ports)
                                       v         v
                               +-------------+  +--------------------------+
                               | agora-core  |  | CaughtUpView over ledger |
                               | AuctionSvc  |  | projections              |
                               +-------------+  +--------------------------+
                                       |
          Inventory, Wallet, MoneySink | AuctionRepository, Clock
                                       v
                               +---------------+       +----------------+
                               | ledger        | ----> | event-sourcing |
                               | AgoraBridge   |       | EventStore     |
                               | LedgerService |       +----------------+
                               +---------------+
```

`tcg` has the same shape with collectors instead of players.

## 2. Crates

Dependencies between workspace crates only, as declared in each `Cargo.toml`.

| Crate | Role | Depends on |
|---|---|---|
| `agora-domain` | Types and pure rules: `Money`, `Bps`, `ItemStack`, `Listing`, `HouseConfig`, `SearchQuery`, errors. | none (external: `serde`, `serde_json`, `thiserror`, `uuid`) |
| `agora-api` | Inbound port: the `AuctionApi` trait and its commands. | `agora-domain` |
| `agora-spi` | Outbound ports: `Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`. | `agora-domain` |
| `agora-core` | `AuctionService`, implements `AuctionApi` on top of the outbound ports. Feature `memory`: in-memory adapters. | `agora-domain`, `agora-api`, `agora-spi` |
| `event-sourcing` | `Decider`, the `EventStore` and `Projection` ports, `execute`, `Projector`. Feature `memory`: in-memory event store. | none |
| `ledger` | Event-sourced accounts and treasury, `LedgerApi`, `LedgerService`, and `AgoraBridge`, which implements Agora's `Inventory`, `Wallet` and `MoneySink` with a `LedgerApi`. | `agora-domain`, `agora-spi`, `event-sourcing` |
| `game-domain` | `Player` (a ledger `Account` whose funds are gold), `CatalogEntry`, `GameError`. | `agora-domain`, `event-sourcing`, `ledger` |
| `game-api` | Inbound port of the game: `GameApi`. | `agora-domain`, `agora-api`, `game-domain` |
| `game-spi` | Outbound ports of the game: `PlayerView`, `TreasuryView`, `Catalog`, `HouseRegistry`, and Agora's `AuctionApi`. | `agora-domain`, `agora-api`, `game-domain` |
| `game-core` | `GameService` (implements `GameApi`), `CaughtUpView`. Feature `memory`: projections and adapters in memory. | `agora-domain`, `agora-api`, `event-sourcing`, `ledger`, `game-domain`, `game-api`, `game-spi` |
| `game-dto` | HTTP requests and responses shared by server and interface. Its rustdoc is the HTTP reference. | `agora-domain` |
| `game-server` | Binary: wiring, HTTP adapter (axum), system clock, settings, settlement loop. | all the crates above except `game-ui` |
| `game-ui` | Web interface (Yew, built to WebAssembly with Trunk). | `agora-domain`, `game-dto` |
| `tcg-*` | Same crates for the trading card game: `tcg-domain`, `tcg-api`, `tcg-spi`, `tcg-core`, `tcg-dto`, `tcg-server`, `tcg-ui`. | same as `game-*` |

Constraints held today:

- `agora-domain`, `agora-api`, `agora-spi` and `agora-core` depend neither on tokio nor on axum. tokio is only a dev-dependency of `agora-core`, for tests.
- `agora-domain` depends on `serde` and `serde_json`: its types are serialized as-is in the HTTP payloads of the demo games, and item attributes are a `serde_json::Value`.
- `event-sourcing` knows no domain.
- The interfaces depend only on `agora-domain` and their DTO crate, so they compile to WebAssembly without the server stack.

## 3. Ports and adapters

| Port | Defined in | Implemented by |
|---|---|---|
| `AuctionApi` | `agora-api` | `AuctionService` (`agora-core`) |
| `Inventory`, `Wallet`, `MoneySink` | `agora-spi` | `AgoraBridge` (`ledger`). Test doubles: `InMemoryInventory`, `InMemoryWallet`, `RecordingMoneySink` (`agora-core`, feature `memory`) |
| `AuctionRepository` | `agora-spi` | `InMemoryAuctionRepository` (`agora-core`, feature `memory`) |
| `Clock` | `agora-spi` | `SystemClock` (each server). Test double: `ManualClock` (`agora-core`, feature `memory`) |
| `EventStore` | `event-sourcing` | `InMemoryEventStore` (`event-sourcing`, feature `memory`) |
| `Projection` | `event-sourcing` | `InMemoryPlayerProjection`, `InMemoryTreasuryProjection` (`game-core`), and their `tcg-core` counterparts |
| `LedgerApi` | `ledger` | `LedgerService` (`ledger`) |
| `PlayerView`, `TreasuryView` | `game-spi` | `CaughtUpView` (`game-core`), and the in-memory projections themselves |
| `Catalog`, `HouseRegistry` | `game-spi` | `StaticCatalog`, `InMemoryHouseRegistry` (`game-core`, feature `memory`) |

Ports are async: their methods return `impl Future<Output = ...> + Send`, and implementations write them as `async fn`. Services are generic over their ports, with no `dyn`.

For a game, Agora is an external system reached through the `AuctionApi` outbound port. Agora in turn drives the game ledger through its own outbound ports, implemented by `AgoraBridge`.

## 4. Use case execution

### Order of effects

`place_bid` and `buyout` run in this order:

1. Load the listing and check the rules.
2. Debit the buyer or bidder. If it fails, nothing has changed.
3. Save the listing with the expected version. On conflict or error, credit the buyer back.
4. Refund the previous high bidder.
5. (Sale only) Deliver the items, pay the seller, send the commission to the sink.

`create_listing` withdraws the stack, then debits the deposit. If the debit fails, the stack is deposited back.

### Concurrency

Optimistic locking, see [ADR 0004](adr/0004-optimistic-locking.md). Each listing has a `version`. `AuctionRepository::update_listing(listing, expected_version)` returns `RepoError::Conflict` when the stored version differs. The use case then runs again from the start, 3 times at most, then fails with `AuctionError::Conflict`. Settlement skips a listing in conflict: it comes back on the next run if it is still due.

### Operation ids

Every call that moves money or items carries an `OpId`, see [ADR 0006](adr/0006-stable-operation-ids.md):

- An effect of a saved listing change (refund, delivery, seller payment, commission, returned stack, lost deposit) gets a UUID v5 derived from the listing id, its saved version and the effect name. Replaying it gives the same `OpId`.
- A debit that starts a new attempt gets a fresh UUID v7, so a retried request is never taken for a replay.
- A compensation gets a UUID v5 derived from the `OpId` of the call it undoes.

The ledger records nothing for an `OpId` it already applied.

## 5. Event sourcing and ledger

`event-sourcing` knows no domain:

- `Decider`: `decide(id, command, state)` returns the events to record (none if the command was already applied). `evolve(state, event)` applies an event and never fails.
- `EventStore<I, E>`: `load` a stream, `append` with the expected number of events (`StoreError::Conflict` otherwise), `read_all` of the global log, positions starting at 1.
- `execute`: loads a stream, folds it, decides and appends. 3 attempts at most on conflict.
- `Projection<I, E>`: read model with a checkpoint. `apply` skips events at or below the checkpoint, so a replayed batch has no effect.
- `Projector::catch_up`: applies the events recorded since the checkpoint, in batches of 256.

`ledger` builds on it, see [ADR 0007](adr/0007-event-sourced-shared-ledger.md):

- One stream per player (`AccountDecider`, `AccountEvent`) and one stream for the treasury (`TreasuryDecider`, `TreasuryId`).
- `LedgerApi`: `open_account`, `withdraw_item`, `deposit_item`, `debit`, `credit`, `take_money`. The state of each stream keeps the set of applied `OpId`s.
- `AgoraBridge` maps Agora's port calls to `LedgerApi` calls and ledger errors to `PortError`. The treasury adds up all houses: the house of a `MoneySink::absorb` call is ignored.

Read side, see [ADR 0008](adr/0008-read-models-caught-up-on-read.md): players and the treasury total are read from projections of the ledger. `CaughtUpView` catches up with the event store on every read, so a read sees every event recorded before the call. The `PlayerView` and `TreasuryView` ports themselves only promise a projection, which may lag.

## 6. Failures

No guarantee today: if a port fails in the middle of a use case, money or items can be lost or duplicated. Agora replays no effect. Examples:

- `create_listing`: if `insert_listing` fails, the stack and the deposit already taken from the seller are not given back.
- After a listing is saved, a failing effect (refund, delivery, payment) returns the error and the remaining effects never run. Settlement only picks Active listings, so it does not retry them either. The `OpId`s prepare the planned outbox described in the [roadmap](roadmap.md).

## 7. Servers

### Wiring

Each server (`game/server/src/main.rs`, `tcg/server/src/main.rs`):

1. reads its settings file (section 8);
2. creates the event stores, the `LedgerService` and opens the starting accounts;
3. builds the `CaughtUpView` read models;
4. builds `AuctionService` with `InMemoryAuctionRepository`, `AgoraBridge` (as inventory, wallet and sink) and `SystemClock`;
5. builds the game service and opens the houses of the settings;
6. spawns the settlement loop, every `settle_every_secs` seconds;
7. serves HTTP.

### HTTP API

Prefix `/api`. Request and response bodies are JSON; their fields and units are documented in the rustdoc of `game-dto` and `tcg-dto`.

| Method and path | Use |
|---|---|
| `GET /catalog` | Item types (`game`) or cards (`tcg`) of the catalog. |
| `GET /houses` | Houses and their terms. |
| `GET /players` (`game`), `GET /collectors` (`tcg`) | Accounts with their money and items. |
| `GET /sink` | Total money taken out of the game. |
| `GET /houses/{house}/listings` | Search. Query: `item_type` (`game`) or `card` (`tcg`), `seller`, `high_bidder`, `limit` (default 50, at most 100), `offset` (default 0). The response carries the server time `now`. |
| `POST /listings` | Sell. |
| `POST /listings/{id}/bid` | Bid. Answers whether the bid leads or bought the listing. |
| `POST /listings/{id}/buyout` | Buy out. `204 No Content`. |
| `POST /listings/{id}/cancel` | Cancel. `204 No Content`. |

Errors answer `{"error": "<message>"}` with:

| Status | When |
|---|---|
| 404 | Unknown house or listing. A search on an unknown house is not an error: it returns an empty page. |
| 409 | Listing version conflict after the retries, or an event store conflict raised by the game service itself. |
| 500 | Repository or storage error, or a port error other than insufficient funds and unavailable item. This includes an unknown player and a ledger conflict after its retries: `AgoraBridge` turns both into `PortError::Other`. |
| 422 | Any other refusal (business rule, validation). |

The messages are listed in [functional.md](functional.md#9-errors).

### Interfaces

Yew apps built with Trunk. `trunk serve` serves `game-ui` on port 3000 and `tcg-ui` on port 3001, and proxies `/api/` to the server address written in their `Trunk.toml`. Each interface polls the server every second.

## 8. Configuration

Each server reads a TOML file from the working directory: `game.toml` or `tcg.toml`, or the file named by `GAME_CONFIG` or `TCG_CONFIG`. A missing file means defaults, but the catalog has no default, so the server refuses to start without one. The shipped files show every default and are checked by a test (`shipped_file_shows_the_defaults`).

| Key | Default | Notes |
|---|---|---|
| `bind_addr` | `127.0.0.1:8080` (`game`), `127.0.0.1:8081` (`tcg`) | `BIND_ADDR` overrides it. |
| `settle_every_secs` | `1` | Above zero. |
| `[log]` | per target levels | `default` sets every other target. `RUST_LOG` overrides the whole table. |
| `[[house]]` | see [functional.md](functional.md#10-demo-games) | At least one. Every key required: `name`, `commission`, `min_increment`, `increment`, `deposit`, `durations` (`minutes`, `deposit_multiplier`). |
| `catalog` | none | `game`: `item_type`, `name`, `base_value`, optional `commodity`. `tcg`: `card_id`, `name`, `rarity`. |
| `rarity` (`tcg`) | common 40, rare 100, epic 400, legendary 1600 | Every rarity required when the table is present. |

Unknown keys are rejected. The server also rejects: duplicate house names, a duration listed twice in a house, a deposit multiplier of 0, an empty or duplicate catalog entry, a base value or rarity value of 0. Agora itself checks the rest of the house terms when the house is opened (see functional.md, section 3).

## 9. Tests

`cargo test --workspace` runs:

- `agora-core`: every business rule, on the in-memory adapters and a manual clock, including that a version conflict during a bid does not debit the bidder twice.
- `ledger`, `game-core`, `tcg-core`: a replayed `OpId` has no effect, and projections rebuild from the events.
- `game-server`, `tcg-server`: settings parsing and validation, and that the shipped TOML files match the defaults.
- `event-sourcing`, `agora-domain`: their own unit tests.

Not covered by automated tests: the interfaces, and settlement end to end through the HTTP API.

## 10. Technical limits

- **Persistence**: the Agora repository, the event stores, the projections and the house registries are in memory. All state is lost on restart.
- **Failures**: see section 6.
- **Idempotence**: the ledger ignores an applied `OpId`, but the in-memory test doubles of `agora-core` do not deduplicate.
- **Ledger growth**: the set of applied `OpId`s of a stream grows forever, and `execute` reloads the whole stream on every command, with no snapshot.
- **Settlement throughput**: 100 listings per run; the rest waits for the next runs.
