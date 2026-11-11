# 0008. Read models caught up on every read

- Status: Accepted
- Date: 2026-10-18

## Context

The players and the treasury total are read from projections of the ledger events ([0007](0007-event-sourced-shared-ledger.md)). A projection fed in the background lags behind the ledger: a player could see their money unchanged right after a purchase.

## Decision

- Projections keep a checkpoint and skip events at or below it, so a replayed batch has no effect.
- `CaughtUpView` wraps a projection and its event store, and catches up with the store (`Projector::catch_up`) before every read.
- The `PlayerView` and `TreasuryView` ports (`CollectorView` in `tcg`) only promise a projection, which may lag. The servers wire `CaughtUpView` behind them.

## Consequences

- A read sees every event recorded before the call.
- Every read pays for the events recorded since the previous one.
- The game core does not depend on how fresh its read models are: another adapter could serve a lagging projection without changing it.
