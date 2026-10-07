# Agora

Rust workspace with three crates:

- `backend`: REST API (axum), listens on port 8080.
- `frontend`: web app (Yew, built with Trunk), served on port 3000.
- `shared`: types shared by backend and frontend.

## Setup

Prerequisites:

- Rust toolchain (via [rustup](https://rustup.rs)).
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or download a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.
- Docker, only for the container setup.

## Run locally

Backend (terminal 1):

```
cargo run -p backend
```

The API is at <http://127.0.0.1:8080/api/health>. Set `BIND_ADDR` to change the listen address (default `127.0.0.1:8080`).

Frontend (terminal 2):

```
cd frontend
trunk serve
```

The app is at <http://127.0.0.1:3000>. `trunk serve` builds the wasm itself, so no separate `cargo build` is needed. Requests to `/api/` are proxied to the backend (see `frontend/Trunk.toml`).

## Run with Docker

```
docker compose up --build
```

- Frontend: <http://localhost:3000> (nginx, proxies `/api/` to the backend).
- Backend: <http://localhost:8080>.

Each service has a multi-stage Dockerfile (`backend/Dockerfile`, `frontend/Dockerfile`). Build context is the repository root.

## Production build of the frontend

```
cd frontend
trunk build --release
```

Output goes to `frontend/dist`.
