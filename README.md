# Agora

Auction house. The auction core is independent of any game: it defines ports, the games implement them. Two demo games trade on it: `game` (gold and stacks of items, commodities bought in part) and `tcg` (a trading card game: dust and cards, one card per listing). See [docs/PRD.md](docs/PRD.md).

Rust workspace. Crates of the auction core live in `agora/`, crates of the demo games (server and web ui) in `game/` and `tcg/`, the generic `event-sourcing` crate in `event-sourcing/`; the crate `agora-core` is in `agora/core`, `game-server` in `game/server`, and so on.

- `agora-domain`: domain types and pure rules (`Money`, `Listing`, `HouseConfig`, errors).
- `agora-api`: inbound ports, the `AuctionApi` use cases.
- `agora-spi`: outbound ports (`Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`).
- `agora-core`: `AuctionService`, implements `agora-api` on top of `agora-spi`. Feature `memory`: in-memory adapters.
- `event-sourcing`: event sourcing independent of any domain: `Decider`, the `EventStore` and `Projection` ports, `execute` and `Projector`. Feature `memory`: in-memory event store.
- `ledger`: event-sourced ledger shared by the demo games: `Account` (funds and items of a player), the account and treasury deciders, `LedgerApi` and `LedgerService`, and `AgoraBridge`, which implements Agora's outbound ports (`Inventory`, `Wallet`, `MoneySink`) with a `LedgerApi`.
- `game-domain`: demo game domain (`Player`, an `Account` holding gold, `CatalogEntry`, `GameError`).
- `game-api`: inbound ports of the game: `GameApi` (player and host use cases).
- `game-spi`: outbound ports of the game (`PlayerView`, `TreasuryView`, `Catalog`, `HouseRegistry`) plus Agora's `AuctionApi`.
- `game-core`: `GameService`, `CaughtUpView`. Feature `memory`: in-memory projections and adapters.
- `game-server`: demo game server. Adapters (HTTP on port 8080, system clock) and wiring.
- `game-dto`: HTTP requests and responses shared by `game-server` and `game-ui`.
- `game-ui`: web app (Yew, built with Trunk), served on port 3000.
- `tcg-*`: the trading card game, same crates as `game-*` on top of `ledger` (`tcg-domain`, `tcg-api`, `tcg-spi`, `tcg-core`, `tcg-server`, `tcg-dto`, `tcg-ui`). A collector holds dust (the money, one unit per dust) and copies of cards, which are interchangeable. A listing always holds a single card, bought whole: `SellCommand` has no quantity, so prices are per card. The base value of a card, used for the deposit, comes from its rarity.

## Setup

Prerequisites:

- Rust toolchain (via [rustup](https://rustup.rs)).
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or download a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.

## Tests

```
cargo test -p agora-domain -p agora-core -p event-sourcing -p ledger -p game-domain -p game-core -p tcg-domain -p tcg-core
```

## Run locally

Game server (terminal 1):

```
cargo run -p game-server
```

The API is under <http://127.0.0.1:8080/api/>. State is in memory and lost on restart.

Settings are read from `game.toml` in the working directory (set `GAME_CONFIG` to read another file): listen address, settlement delay (every second by default), log levels per target, auction houses (`[[house]]`), item catalog (`catalog` array). Every key is optional except `catalog`, which has no default: without a catalog the server refuses to start. Each catalog entry may set `commodity = true`: a commodity (resource, consumable) is sold at a fixed price only and can be bought in part, the seller getting back the matching share of the deposit; any other item is auctioned and bought whole. Environment variables override the file: `BIND_ADDR` for the listen address, `RUST_LOG` for the whole log table. By default there are two auction houses, "Alliance" (5% commission) and "Neutre" (15% commission), and three players (`alice`, `bob`, `carol`) start with 50 to 200 gold and 1 to 3 random item types. Amounts are in copper (1 gold = 100 silver = 10,000 copper).

Logs go to stdout. With the default log table, they show business events, rejected requests, settlement and every ledger call Agora makes to the game (with its `OpId`). Set `tower_http` to `debug` to also log every HTTP response, or every level to `info` to keep only business events, rejected requests and settlement.

UI (terminal 2):

```
cd game/ui
trunk serve
```

The app is at <http://127.0.0.1:3000> and refreshes every second. Requests to `/api/` are proxied to the game server (see `game/ui/Trunk.toml`). Pick a player in the header: there is no authentication.

## Run the trading card game

Same steps as the game, on other ports so both can run at once:

```
cargo run -p tcg-server
```

The API is under <http://127.0.0.1:8081/api/>. Settings are read from `tcg.toml` (set `TCG_CONFIG` to read another file): the same keys as `game.toml`, plus `rarity` (value of one card in dust per rarity), and a `catalog` of cards (`card_id`, `name`, `rarity`). The `rarity` table is optional (defaults: common 40, rare 100, epic 400, legendary 1600); when present, it must set every rarity to a value greater than zero. By default there is one auction house, "Marché" (5% commission), and three collectors (`alice`, `bob`, `carol`) start with 1000 to 5000 dust and 3 to 6 random cards, 1 to 3 copies each.

```
cd tcg/ui
trunk serve
```

The app is at <http://127.0.0.1:3001>.

## Production build of the UI

```
cd game/ui
trunk build --release
```

Output goes to `game/ui/dist`. Same for the trading card game in `tcg/ui`, output in `tcg/ui/dist`.
