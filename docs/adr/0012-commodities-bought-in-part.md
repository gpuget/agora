# 0012. Commodities bought in part, other items auctioned whole

- Status: Accepted
- Date: 2026-11-02

## Context

Items fall into two kinds: resources and consumables, traded in stacks, and equipment, rare items and cards. Until this decision, every listing was auctioned and its buyout took the whole stack.

## Decision

- `ItemStack` carries a `commodity` flag, set by the game (the `commodity` key of the `game` catalog).
- A commodity takes no bids and needs a buyout price. It can be bought in any quantity up to what is left; the listing stays open with the rest.
- Any other item is auctioned and bought whole.
- On a partial sale, the seller gets back the share of the deposit matching the quantity sold, rounded down. The last part gives back all the deposit still held. The rest is lost on cancellation or expiry.
- `buyout` takes a quantity.

## Consequences

- A commodity can end `Sold` after several buyers. `ListingStatus::Sold` records only the last buyer and price.
- A commodity sold in part that reaches its end time ends `Expired`, like a listing without a bid.
- The `game` interface hides the bid input and the start price for commodities, and shows a quantity input for the buyer.
