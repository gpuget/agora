# Agora

Auction house that any game can plug in. Players sell, bid and buy through their game, Agora holds items and money in escrow and settles the sales. Two demo games trade on it: `game` (gold and stacks of items) and `tcg` (a trading card game with dust and cards).

## Documentation

| Document | Content |
|---|---|
| [docs/functional.md](docs/functional.md) | What Agora does: glossary, listings and their lifecycle, business rules, money flows, errors, demo games. |
| [docs/architecture.md](docs/architecture.md) | How it is built: crates, ports and adapters, concurrency, event sourcing, HTTP API, configuration, tests. |
| [docs/adr/](docs/adr/) | Why: one record per decision in force. |
| [docs/roadmap.md](docs/roadmap.md) | What is planned and what is out of scope. |
| [CHANGELOG.md](CHANGELOG.md) | Notable changes per version. |
| rustdoc | API of every crate: `cargo doc --no-deps --workspace --open`. The HTTP payloads are documented in `game-dto` and `tcg-dto`. |

## Setup

Prerequisites:

- Rust toolchain via [rustup](https://rustup.rs). The workspace uses edition 2024 and let chains, so Rust 1.88 at least; it is developed with Rust 1.99.
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.

## Tests

```
cargo test --workspace
```

## Run the game

Run every command from the repository root: the servers read their settings file from the working directory.

Server (terminal 1):

```
cargo run -p game-server
```

The API is under <http://127.0.0.1:8080/api/>. State is in memory and lost on restart.

Interface (terminal 2):

```
cd game/ui
trunk serve
```

The app is at <http://127.0.0.1:3000>. Pick a player in the header: there is no authentication. Trunk proxies `/api/` to the address in `game/ui/Trunk.toml`.

## Run the trading card game

Same steps on other ports, so both games can run at once:

```
cargo run -p tcg-server
```

```
cd tcg/ui
trunk serve
```

The API is under <http://127.0.0.1:8081/api/>, the app at <http://127.0.0.1:3001>.

## Configuration

`game-server` reads `game.toml` and `tcg-server` reads `tcg.toml`. Set `GAME_CONFIG` or `TCG_CONFIG` to read another file. The shipped files list every key with its default and its constraints. In short:

- Every top-level key is optional except `catalog`. Without a catalog the server refuses to start.
- Inside a `[[house]]` table, a catalog entry or the `rarity` table, every key is required, except `commodity` in the `game` catalog.
- `BIND_ADDR` overrides the listen address, `RUST_LOG` the whole log table.
- If you change the listen address, change the proxy `backend` in the `Trunk.toml` of the interface too.

Full reference: [docs/architecture.md, section 8](docs/architecture.md#8-configuration).

## Logs

Logs go to stdout. With the default log table, they show business events, rejected requests, settlement and every ledger call Agora makes (with its `OpId`). Set `tower_http` to `debug` to also log every HTTP response, or every level to `info` to keep only business events, rejected requests and settlement.

## Production build of the interfaces

```
cd game/ui
trunk build --release
```

Output goes to `game/ui/dist`. Same in `tcg/ui`, output in `tcg/ui/dist`.
