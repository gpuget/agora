# 0007. Event-sourced ledger shared by the demo games

- Status: Accepted
- Date: 2026-10-18

## Context

The demo games need to record the money and items of each player, and the money Agora takes out of the game, in a way that ignores replayed calls ([0006](0006-stable-operation-ids.md)). The ledgers of `game` and `tcg` turned out to be the same code under other names.

## Decision

- A domain-independent crate, `event-sourcing`, provides `Decider` (decide / evolve), the `EventStore` and `Projection` ports, `execute` and `Projector`.
- A `ledger` crate, shared by both games, holds one stream per account (`AccountDecider`) and one stream for the treasury (`TreasuryDecider`). Each movement records the `OpId` of the Agora call; a replayed call records nothing.
- Writes go through `execute`: load the stream, decide, append with the expected number of events, retry on conflict (3 attempts).
- `AgoraBridge` implements Agora's `Inventory`, `Wallet` and `MoneySink` with a `LedgerApi`.
- `Player` (`game`) and `Collector` (`tcg`) are aliases of the ledger `Account`. Ledger errors live in `LedgerError`; `GameError` and `TcgError` keep the game-level cases.

## Consequences

- Both games get the same ledger behaviour and tests.
- The history of every account is kept as events. Read models are projections ([0008](0008-read-models-caught-up-on-read.md)).
- `execute` reloads the whole stream on every command, with no snapshot.
- The ledger counts items per item type: item attributes do not survive a withdraw and a deposit.
- The treasury adds up all houses of a game.
