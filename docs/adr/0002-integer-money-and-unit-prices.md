# 0002. Integer money, rates in basis points rounded down, prices per unit

- Status: Accepted
- Date: 2026-10-14

## Context

Agora handles the money of games with different currencies: copper in `game`, dust in `tcg`. Listings hold stacks of several items.

## Decision

- An amount is `Money(u64)` in the smallest unit of the game currency. Formatting it (gold, silver, copper) is up to the game.
- A rate is `Bps(u32)` in basis points: 500 is 5 %, 10,000 is 100 %. Every rate computation rounds down (`Money::apply_bps`).
- Every price of a listing is a unit price. A total is `unit price * quantity`. All arithmetic is checked: an overflow is refused with `AuctionError::Overflow`.

## Consequences

- No floating point anywhere in money computations.
- Rounding always favours the payer of a rate: commission, deposit and increments are rounded down. A small deposit can round down to 0 and then is not taken.
- A partial sale gives back a deposit share rounded down. To lose nothing to rounding, the last part gives back all the deposit still held ([0012](0012-commodities-bought-in-part.md)).
- The interfaces convert a price typed for a whole stack into a unit price, with rounding of their own.
