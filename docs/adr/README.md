# Architecture decision records

Each record states one decision in force today: its context, the decision and its consequences. A record is never rewritten. When a decision changes, a new record supersedes it and the old one gets the status `Superseded by NNNN`. Planned work that is not decided yet goes in [../roadmap.md](../roadmap.md).

Dates are those of the commit that made the decision.

| # | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-hexagonal-auction-core.md) | Hexagonal auction core, independent of any game | Accepted | 2026-10-14 |
| [0002](0002-integer-money-and-unit-prices.md) | Integer money, rates in basis points rounded down, prices per unit | Accepted | 2026-10-14 |
| [0003](0003-trusted-game-server.md) | The game server is the only, trusted client of Agora | Accepted | 2026-10-14 |
| [0004](0004-optimistic-locking.md) | Optimistic locking on listings, with bounded retries | Accepted | 2026-10-14 |
| [0005](0005-escrow-and-deposit.md) | Escrow of items and bids, deposit paid by the seller | Accepted | 2026-10-14 |
| [0006](0006-stable-operation-ids.md) | Stable operation ids on every movement | Accepted | 2026-10-18 |
| [0007](0007-event-sourced-shared-ledger.md) | Event-sourced ledger shared by the demo games | Accepted | 2026-10-18 |
| [0008](0008-read-models-caught-up-on-read.md) | Read models caught up on every read | Accepted | 2026-10-18 |
| [0009](0009-in-memory-storage.md) | In-memory storage for the prototype | Accepted | 2026-10-14 |
| [0010](0010-toml-settings.md) | Server settings in a TOML file, catalog required | Accepted | 2026-10-18 |
| [0011](0011-one-card-per-listing.md) | One card per listing in the trading card game | Accepted | 2026-10-18 |
| [0012](0012-commodities-bought-in-part.md) | Commodities bought in part, other items auctioned whole | Accepted | 2026-11-02 |

## Template

```
# NNNN. Title

- Status: Proposed | Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context

## Decision

## Consequences
```
