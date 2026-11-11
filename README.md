# Agora

Auction house that any game can plug in. Players sell, bid and buy through their game; Agora holds items and money in escrow and settles the sales. Two demo games trade on it: `game` (gold and stacks of items) and `tcg` (a trading card game with dust and cards).

Status: prototype. State is kept in memory only.

## Where to start

| You want to | Read |
|---|---|
| Run the demo and try a sale | [docs/getting-started.md](docs/getting-started.md) |
| Know the exact rules: listings, bids, deposits, commissions | [docs/functional.md](docs/functional.md) |
| Plug Agora into your own game | [docs/integrate-a-game.md](docs/integrate-a-game.md) |
| Call the HTTP API of a demo server | rustdoc of `game-dto` and `tcg-dto`: `cargo doc --no-deps -p game-dto -p tcg-dto --open` |
| Change the settings of a demo server | [docs/configuration.md](docs/configuration.md) |
| Understand how the code is built | [docs/architecture.md](docs/architecture.md) |
| Know why it is built that way | [docs/adr/](docs/adr/) |
| Know what is planned | [docs/roadmap.md](docs/roadmap.md) |
| Contribute | [CONTRIBUTING.md](CONTRIBUTING.md) |
| See what changed | [CHANGELOG.md](CHANGELOG.md) |

## Quick start

From the repository root, with Rust, the `wasm32-unknown-unknown` target and Trunk installed:

```
cargo run -p game-server
```

```
cd game/ui
trunk serve
```

Then open <http://127.0.0.1:3000>. Details and prerequisites: [docs/getting-started.md](docs/getting-started.md).
