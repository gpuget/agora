# 0004. Optimistic locking on listings, with bounded retries

- Status: Accepted
- Date: 2026-10-14

## Context

Several players can bid on, buy or cancel the same listing at the same time, while settlement closes it.

## Decision

- Each listing carries a `version`, incremented on every save.
- `AuctionRepository::update_listing(listing, expected_version)` fails with `RepoError::Conflict` if the stored version changed.
- A use case that hits a conflict runs again from the start, 3 attempts at most, then fails with `AuctionError::Conflict`.
- When a save fails after the buyer was debited, the buyer is credited back.
- Settlement skips a listing in conflict; it comes back on the next run if still due.

The event store of the ledger applies the same principle per stream ([0007](0007-event-sourced-shared-ledger.md)).

## Consequences

- No lock is held across port calls.
- Under heavy contention a request can fail with `listing changed concurrently, try again` (HTTP 409).
- A debit can happen and be compensated within one attempt, so the ledger sees both movements.
