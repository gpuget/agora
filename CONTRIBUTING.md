# Contributing

For whoever changes the code or the documentation. To install the tools and run the demo, follow [docs/getting-started.md](docs/getting-started.md). To understand the code, read [docs/architecture.md](docs/architecture.md).

## Tests

```
cargo test --workspace
```

What runs:

- `agora-core`: every business rule, on the in-memory adapters and a manual clock, including that a version conflict during a bid does not debit the bidder twice.
- `ledger`, `game-core`, `tcg-core`: a replayed `OpId` has no effect, and projections rebuild from the events.
- `game-server`, `tcg-server`: settings parsing and validation, and `shipped_file_shows_the_defaults`, which fails when `game.toml` or `tcg.toml` no longer matches the defaults in the code. Change both together.
- `event-sourcing`, `agora-domain`: their own unit tests.

Not covered: the interfaces, and settlement end to end through the HTTP API. Check those by hand, following the getting started tour.

A new business rule comes with a test in `agora/core/src/tests.rs`.

## Formatting

Format with rustfmt (edition 2024). A few files are not formatted yet, so format the files you change rather than the whole workspace:

```
rustfmt --edition 2024 path/to/file.rs
```

## Commits

- [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), in English: `feat`, `fix`, `refactor`, `docs`, `chore`, with an optional scope such as `agora`, `ledger`, `game`, `tcg`, `ui`.
- One topic per commit. The body says what changed and why.
- A breaking change of a public API says so in the body and in the changelog.

## Documentation

Each fact lives in one place. Update it in the same commit as the code.

| What changed | Where to write it |
|---|---|
| A business rule, a status, a term | [docs/functional.md](docs/functional.md) |
| How to run the demo | [docs/getting-started.md](docs/getting-started.md) |
| What a game must provide to Agora | [docs/integrate-a-game.md](docs/integrate-a-game.md) |
| A settings key or its rules | [docs/configuration.md](docs/configuration.md) and the comments of `game.toml` / `tcg.toml` |
| A value of a demo game (house, rate, catalog) | `game.toml` / `tcg.toml`. [docs/getting-started.md](docs/getting-started.md) quotes the shipped values: check it when you change them. |
| Crates, ports, concurrency, failures | [docs/architecture.md](docs/architecture.md) |
| A public type, function, route or payload | its rustdoc. The HTTP API is documented in `game-dto` and `tcg-dto`. |
| A design decision | a new record in [docs/adr/](docs/adr/). Never rewrite an accepted one: supersede it. |
| Planned work or a known limit with no plan | [docs/roadmap.md](docs/roadmap.md) |
| Anything a user of a version would notice | `Unreleased` in [CHANGELOG.md](CHANGELOG.md) |

Rustdoc: document what the signature does not say (units, invariants, errors, side effects). Leave self-evident fields alone. `cargo doc --no-deps --workspace` must stay free of warnings.

Write in English, without emojis.

## Releases

All crates share the workspace version in `Cargo.toml`. To release:

1. Move the `Unreleased` entries of the changelog under the new version and date.
2. Tag the commit with the version, such as `0.3.0`.
3. Bump the workspace version to the next one in a `chore: bump version to X.Y.Z` commit.

The workspace version is therefore the version being prepared, not the last tag.
