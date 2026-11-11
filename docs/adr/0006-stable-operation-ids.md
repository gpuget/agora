# 0006. Stable operation ids on every movement

- Status: Accepted
- Date: 2026-10-18

## Context

Every port call got a fresh `OpId`, so a replayed effect could never be recognized by the game ledger. Replaying effects is needed for the planned outbox ([roadmap](../roadmap.md)).

## Decision

Every `Inventory`, `Wallet` and `MoneySink` call carries an `OpId`:

- An effect of a saved listing change (bid refund, delivery, seller payment, commission, returned stack, lost deposit) gets a UUID v5 derived from the listing id, its saved version and the effect name.
- A compensation gets a UUID v5 derived from the `OpId` of the call it undoes.
- A debit that starts a new attempt gets a fresh UUID v7, so a retried request is never mistaken for a replay.

The ledger records nothing for an `OpId` it already applied.

## Consequences

- Replaying an effect is safe on the ledger side.
- Agora does not replay anything yet: the ids only pay off once the outbox exists.
- The ledger keeps the set of applied `OpId`s per stream, and this set is never purged.
