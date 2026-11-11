# Integrate Agora into a game

For a developer who wants their own game server to trade on Agora. The two demo servers do exactly this: `game/server/src/main.rs` is the reference wiring, and this page walks through it.

Read [functional.md](functional.md) first for the rules, and [ADR 0003](adr/0003-trusted-game-server.md) for the trust model: your server is Agora's only client, and players never call Agora directly.

## 1. Add the crates

The crates are not published: depend on them by path.

```toml
[dependencies]
agora-domain = { path = "<agora>/agora/domain" }
agora-api = { path = "<agora>/agora/api" }
agora-spi = { path = "<agora>/agora/spi" }
# `memory` provides InMemoryAuctionRepository, the only repository today.
agora-core = { path = "<agora>/agora/core", features = ["memory"] }
```

Agora needs an async runtime but does not pick one. The demo servers use tokio.

## 2. Give Agora access to your players

Agora moves money and items through three outbound ports of `agora-spi`: `Inventory`, `Wallet` and `MoneySink`. Two ways to provide them.

### Option A: reuse the shared ledger

The `ledger` crate keeps each player's money and items as events, and `AgoraBridge` implements the three ports on top of it. Add `ledger` and `event-sourcing` (with its `memory` feature), then:

```rust
use agora_domain::{Money, PlayerRef};
use event_sourcing::memory::InMemoryEventStore;
use ledger::{Account, AccountEvent, AgoraBridge, LedgerApi, LedgerService, TreasuryEvent, TreasuryId};

let accounts = InMemoryEventStore::<PlayerRef, AccountEvent>::default();
let treasury = InMemoryEventStore::<TreasuryId, TreasuryEvent>::default();
let ledger = LedgerService::new(accounts.clone(), treasury.clone());

// Every player needs an account before trading.
ledger
    .open_account(Account::new("alice".into(), Money(1_000_000)).with_item("copper_ore".into(), 20))
    .await?;

let bridge = AgoraBridge(ledger);
```

Keep the two event stores: your read models (balances, inventories, money taken out of the game) are projections of them. See `CaughtUpView` in `game-core` and [ADR 0008](adr/0008-read-models-caught-up-on-read.md).

### Option B: implement the ports yourself

Implement `Inventory`, `Wallet` and `MoneySink` on your own player storage. Their rustdoc in [agora/spi/src/lib.rs](../agora/spi/src/lib.rs) states when each call happens. The contract:

- `Inventory::withdraw_item` fails with `PortError::ItemUnavailable` when the player does not hold the stack. `Wallet::debit` fails with `PortError::InsufficientFunds` when the balance is too low. Agora turns these into refusals for the player.
- Any other failure is `PortError::Other`. Agora gives no guarantee when a port fails in the middle of a use case ([architecture.md, section 6](architecture.md#6-failures)).
- Every call carries an `OpId`. Ignore an `OpId` you already applied, so a replayed call has no effect ([ADR 0006](adr/0006-stable-operation-ids.md)).
- `MoneySink::absorb` receives commissions and lost deposits, always above zero, with the house and the reason. What happens to that money is your decision.
- `ItemStack::attributes` is opaque to Agora. Keep it in your inventory if your items need it.

## 3. Provide a clock

Implement `agora_spi::Clock`: `now()` returns a `Timestamp`, in milliseconds since the Unix epoch. The demo servers use the system time (`game/server/src/adapters/clock.rs`). Tests can use `ManualClock` from the `memory` feature of `agora-core`.

## 4. Build the service

```rust
use agora_core::AuctionService;
use agora_core::memory::InMemoryAuctionRepository;

let agora = AuctionService::new(
    InMemoryAuctionRepository::default(),
    bridge.clone(), // Inventory
    bridge.clone(), // Wallet
    bridge,         // MoneySink
    SystemClock,
);
```

`AuctionService` implements `agora_api::AuctionApi`, the only entry point you call. Each method's rustdoc ([agora/api/src/lib.rs](../agora/api/src/lib.rs)) lists its effects and its errors.

## 5. Open the houses

Call `create_auction_house` once per house at startup, with a `HouseConfig`: name, commission, minimum increment, deposit rate and allowed durations. Keep the returned `HouseId`: Agora has no call that lists houses, so your server keeps its own registry (the demo games use `HouseRegistry`).

## 6. Act for your players

Each player action maps to one call. Pass the `PlayerRef` of the player your server authenticated, never one sent by the client.

| Player action | Call |
|---|---|
| Sell | `create_listing(CreateListing { house, seller, item, start_unit_price, buyout_unit_price, duration })` |
| Bid | `place_bid(bidder, listing, unit_price)` |
| Buy out | `buyout(buyer, listing, quantity)` |
| Cancel | `cancel_listing(seller, listing)` |
| Browse | `search(SearchQuery { house, item_type, seller, high_bidder, limit, offset })` |

Build the `ItemStack` of a listing on the server side: `base_value` sets the deposit and `commodity` decides between fixed price and auction, so both must come from your catalog, not from the request. See `GameService::sell` in `game/core/src/service.rs`.

## 7. Settle expired listings

Agora never wakes up by itself. Call `settle_expired` at regular intervals; each call settles at most 100 listings. The demo servers run it every second:

```rust
// Share the service between the settlement task and your request handlers.
let agora = std::sync::Arc::new(agora);
let settler = agora.clone();
tokio::spawn(async move {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tick.tick().await;
        if let Err(error) = settler.settle_expired().await {
            tracing::error!(%error, "settlement failed");
        }
    }
});
```

## 8. Report errors to players

Every call returns `AuctionError`. Its rustdoc ([agora/domain/src/error.rs](../agora/domain/src/error.rs)) says when each variant is returned. The demo servers map them to HTTP statuses: not found, conflict (retry), refused by a rule, or server error. See `ApiError` in `game/server/src/adapters/http.rs`.

## Before going to production

Agora is a prototype. The repository is in memory only, and a port failure in the middle of a use case can lose money or items. Both are on the [roadmap](roadmap.md).
