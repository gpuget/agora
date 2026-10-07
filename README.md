# Agora

Rust workspace with three crates:

- `agora-api`: REST API (axum), listens on port 8080.
- `agora-web`: web app (Yew, built with Trunk), served on port 3000.
- `agora-shared`: types shared by agora-api and agora-web.

## Setup

Prerequisites:

- Rust toolchain (via [rustup](https://rustup.rs)).
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or download a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.
- Docker, only for the container setup.

## Run locally

API (terminal 1):

```
cargo run -p agora-api
```

The API is at <http://127.0.0.1:8080/api/health>. Set `BIND_ADDR` to change the listen address (default `127.0.0.1:8080`).

Web (terminal 2):

```
cd agora-web
trunk serve
```

The app is at <http://127.0.0.1:3000>. `trunk serve` builds the wasm itself, so no separate `cargo build` is needed. Requests to `/api/` are proxied to the API (see `agora-web/Trunk.toml`).

## Run with Docker

```
docker compose up --build
```

- Web: <http://localhost:3000> (nginx, proxies `/api/` to the API).
- API: <http://localhost:8080>.

Each service has a multi-stage Dockerfile (`agora-api/Dockerfile`, `agora-web/Dockerfile`). Build context is the repository root.

## Production build of the web app

```
cd agora-web
trunk build --release
```

Output goes to `agora-web/dist`.
