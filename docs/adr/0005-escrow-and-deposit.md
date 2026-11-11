# 0005. Escrow of items and bids, deposit paid by the seller

- Status: Accepted
- Date: 2026-10-14

## Context

A listing stays open for minutes while players bid on it, then settlement delivers the items to the winner and pays the seller. Meanwhile the seller and the bidders keep using their items and money in the game.

## Decision

- Creating a listing withdraws the stack from the seller's inventory and debits a deposit from the seller.
- A bid debits its total from the bidder at once. The previous high bidder is credited back.
- The deposit is `base value * quantity * deposit rate * duration multiplier`, fixed at creation. It is given back when the items sell and lost (sent to the `MoneySink`) when they do not.
- A listing copies the commission, minimum increment and deposit of its house at creation.

## Consequences

- Settlement only moves what is already held: it cannot fail for lack of funds or items.
- A seller pays to list, more for a longer duration. Listing items that do not sell has a cost.
- A bidder's money is locked while they lead.
- Changing the terms of a house does not affect the listings already open.
