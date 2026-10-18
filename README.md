# Agora

Auction house inspired by World of Warcraft. The auction core is independent of any game: it defines ports, the game implements them. See [docs/PRD.md](docs/PRD.md).

Rust workspace. Crates of the auction core live in `agora/`, crates of the demo game (server and web ui) in `game/`; the crate `agora-core` is in `agora/core`, `game-server` in `game/server`, and so on.

- `agora-domain`: domain types and pure rules (`Money`, `Listing`, `HouseConfig`, errors).
- `agora-api`: inbound ports, the `AuctionApi` use cases.
- `agora-spi`: outbound ports (`Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`).
- `agora-core`: `AuctionService`, implements `agora-api` on top of `agora-spi`. Feature `memory`: in-memory adapters.
- `game-domain`: demo game domain (`Player`, `CatalogEntry`, `GameError`).
- `game-api`: inbound ports of the game: `GameApi` (player use cases) and `LedgerApi` (gold and item movements requested by Agora).
- `game-spi`: outbound ports of the game (`EventStore`, `Projection`, `PlayerView`, `TreasuryView`, `Catalog`, `HouseRegistry`) plus Agora's `AuctionApi`.
- `game-core`: `GameService` and `LedgerService` (event-sourced ledger), `Projector` and `CaughtUpView`. Feature `memory`: in-memory adapters.
- `game-server`: demo game server. Adapters (HTTP on port 8080, bridge implementing Agora's outbound ports with `LedgerApi`, system clock) and wiring.
- `game-dto`: HTTP requests and responses shared by `game-server` and `game-ui`.
- `game-ui`: web app (Yew, built with Trunk), served on port 3000.

## Setup

Prerequisites:

- Rust toolchain (via [rustup](https://rustup.rs)).
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or download a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.

## Tests

```
cargo test -p agora-domain -p agora-core -p game-domain -p game-core
```

## Run locally

Game server (terminal 1):

```
cargo run -p game-server
```

The API is under <http://127.0.0.1:8080/api/>. State is in memory and lost on restart.

Settings are read from `game.toml` in the working directory (set `GAME_CONFIG` to read another file): listen address, settlement delay (every second by default), log levels per target, auction houses (`[[house]]`), item catalog (`[[catalog]]`). Every key is optional except `[[catalog]]`, which has no default: without a catalog the server refuses to start. Environment variables override the file: `BIND_ADDR` for the listen address, `RUST_LOG` for the whole log table.

Logs go to stdout. With the default log table, they show business events, rejected requests, settlement and every ledger call Agora makes to the game (with its `OpId`). Set `tower_http` to `debug` to also log every HTTP response, or every level to `info` to keep only business events, rejected requests and settlement.

UI (terminal 2):

```
cd game/ui
trunk serve
```

The app is at <http://127.0.0.1:3000>. Requests to `/api/` are proxied to the game server (see `game/ui/Trunk.toml`). Pick a player in the header: there is no authentication.

## Production build of the UI

```
cd game/ui
trunk build --release
```

Output goes to `game/ui/dist`.
