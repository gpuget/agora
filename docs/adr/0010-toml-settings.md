# 0010. Server settings in a TOML file, catalog required

- Status: Accepted
- Date: 2026-10-18

## Context

The listen address, the log levels, the auction houses and the item catalog of the demo servers were constants in `main.rs`.

## Decision

- Each server reads a TOML file from the working directory: `game.toml` or `tcg.toml`, or the file named by `GAME_CONFIG` or `TCG_CONFIG`.
- Every top-level key is optional except the catalog. A missing file means defaults. Unknown keys are rejected.
- The catalog has no default: without one, the server refuses to start.
- `BIND_ADDR` and `RUST_LOG` override the file.
- The server validates the file (unique names, positive values) before starting. The shipped files show every default, and a test checks that they match.

## Consequences

- The servers must be started from the repository root, or with `GAME_CONFIG` / `TCG_CONFIG` set: elsewhere they find no file and refuse to start for lack of a catalog.
- The proxy address of each interface is written in its `Trunk.toml`, not read from these files: changing `bind_addr` or `BIND_ADDR` requires changing it too.
