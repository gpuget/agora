# Agora

Auction house inspired by World of Warcraft. The auction core is independent of any game: it defines ports, the game implements them. See [docs/PRD.md](docs/PRD.md).

Rust workspace:

- `agora-domain`: domain types and pure rules (`Money`, `Listing`, `HouseConfig`, errors).
- `agora-api`: inbound ports, the `AuctionApi` use cases.
- `agora-spi`: outbound ports (`Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock`).
- `agora-core`: `AuctionService`, implements `agora-api` on top of `agora-spi`. Feature `memory`: in-memory adapters.
- `game`: demo game server. Implements the outbound ports, hosts the core, exposes HTTP on port 8080.
- `game-dto`: HTTP requests and responses shared by `game` and `ui`.
- `ui`: web app (Yew, built with Trunk), served on port 3000.

## Setup

Prerequisites:

- Rust toolchain (via [rustup](https://rustup.rs)).
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or download a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.

## Tests

```
cargo test -p agora-domain -p agora-core
```

## Run locally

Game server (terminal 1):

```
cargo run -p game
```

The API is under <http://127.0.0.1:8080/api/>. Set `BIND_ADDR` to change the listen address (default `127.0.0.1:8080`). State is in memory and lost on restart. Expired listings are settled every second.

UI (terminal 2):

```
cd ui
trunk serve
```

The app is at <http://127.0.0.1:3000>. Requests to `/api/` are proxied to the game server (see `ui/Trunk.toml`). Pick a player in the header: there is no authentication.

## Production build of the UI

```
cd ui
trunk build --release
```

Output goes to `ui/dist`.
