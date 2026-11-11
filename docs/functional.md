# Agora: functional documentation

What Agora does for the games that use it and for their players. This document describes the current behaviour. How it is built is in [architecture.md](architecture.md), why it is built that way in [adr/](adr/), and what is planned in [roadmap.md](roadmap.md).

## 1. Purpose and actors

Agora is an auction house that any game can plug in. Players put items up for sale, bid on them or buy them at a fixed price. Agora holds the items and money in escrow while a sale is open, and settles it when it ends.

| Actor | Role |
|---|---|
| Game server | The only client of Agora. It is trusted: it says who the player is and what an item is worth. Players never call Agora directly. |
| Player | Sells, bids, buys and cancels through the game interface. Called a collector in the trading card game. |
| Game host | Opens the auction houses at startup and triggers the settlement of expired listings at regular intervals. |

## 2. Glossary

One term per concept. The last column names the matching type or field in the code.

| Term | Meaning | In code |
|---|---|---|
| Auction house (house) | A market with its own terms. Houses are isolated: a listing belongs to one house and is only visible there. | `AuctionHouse`, `HouseConfig` |
| Listing | One stack put up for sale in a house. | `Listing` |
| Stack | The items of a listing: one item type and a quantity. | `ItemStack` |
| Item type | Identifies a kind of item, defined by the game. A card in the trading card game. | `ItemType` |
| Commodity | An item type sold at a fixed price only, which can be bought in part (resources, consumables). | `ItemStack::commodity` |
| Auctioned item | Any item type that is not a commodity. It receives bids and is bought whole. | `commodity = false` |
| Seller | The player who created the listing. | `Listing::seller` |
| Bidder | A player who places a bid. | `Bid::bidder` |
| High bid | The best bid on a listing so far. | `Listing::high_bid` |
| Buyer | A player who buys items, at the buyout price or by winning the auction. | `ListingStatus::Sold::buyer` |
| Unit price | Price of one item. Every price in Agora is a unit price. The total is unit price times quantity. | `*_unit_price` |
| Start price | Minimum unit price of the first bid. | `start_unit_price` |
| Buyout price | Fixed unit price at which the items can be bought immediately. Optional for an auctioned item, required for a commodity. | `buyout_unit_price` |
| Minimum increment | How much a new bid must add to the high bid. | `min_increment`, `increment` |
| Duration | How long a listing stays open. Each house allows a fixed list of durations. | `DurationOption` |
| Base value | Value of one item, set by the game. Only used to compute the deposit. | `base_value` |
| Deposit | Money taken from the seller when the listing is created. Given back when the items sell, lost otherwise. | `Listing::deposit` |
| Deposit share | The part of the deposit given back for a partial sale of a commodity. | `Listing::deposit_share` |
| Commission | Part of the sale price kept by the house. | `commission` |
| Escrow | Items and money held by Agora while a listing is open: the stack, and the money of the high bidder. | |
| Settlement | Closing the listings whose end time has passed: deliver to the winner or return to the seller. | `settle_expired` |
| Sink | Where the money taken out of the game goes: commissions and lost deposits. The game decides what to do with it. The demo games record it in a treasury. | `MoneySink`, `TreasuryId` |
| Money | An amount in the smallest unit of the game currency: copper in `game`, dust in `tcg`. | `Money` |
| Basis point (bps) | Unit of the rates: 100 bps = 1 %, 10,000 bps = 100 %. | `Bps` |

## 3. Auction houses

A game opens as many houses as it wants, for example one per faction and a neutral one. Each house sets:

- a **commission** rate, at most 100 %;
- a **deposit** rate;
- a **minimum increment**: an absolute amount per unit (at least 1) and a rate;
- the allowed **durations** (at least one, each greater than zero), each with a **deposit multiplier**.

A listing copies the commission, the minimum increment and the deposit of its house when it is created. Later changes to the house do not affect it.

## 4. Listings

A listing holds one stack of one item type. The seller sets a start price, an optional buyout price and a duration allowed by the house.

- **Commodity**: no bids. The buyout price is required. Buyers take any quantity from 1 to what is left, and the listing stays open with the rest.
- **Auctioned item**: receives bids. A buyout price is optional. The whole stack goes to one buyer.

The game decides which item types are commodities.

### Lifecycle

```
                 bid (stays Active, high bid replaced)
                 partial buyout of a commodity (stays Active, quantity reduced)
                   +----+
                   |    v
  create ----> Active ----------------------------> Sold
                 |   buyout of all that is left,
                 |   bid at or above the buyout price,
                 |   or settlement with a high bid
                 |
                 +---- settlement without a bid -----> Expired
                 |
                 +---- cancel by the seller, no bid --> Cancelled
```

| Status | Meaning |
|---|---|
| Active | Open. Once its end time has passed, it accepts nothing more and waits for settlement, but it still shows as Active until then. |
| Sold | Everything was sold. The status records one buyer and one unit price: the winner of the auction, the buyer at the buyout price, or, for a commodity sold in parts, the buyer of the last part. |
| Expired | Ended without a bid. A commodity sold in part that reaches its end time also ends Expired: what was left goes back to the seller. |
| Cancelled | Cancelled by the seller before any bid. |

Sold, Expired and Cancelled are final.

## 5. Use cases

| Use case | Who | Effect |
|---|---|---|
| Open a house | Game host | Checks the terms (section 3) and creates the house. |
| Sell | Player | Takes the stack from the seller's inventory into escrow, takes the deposit from the seller's money, creates the listing. If the deposit cannot be paid, the stack goes back to the seller. |
| Bid | Player | Takes the total of the bid (unit price times quantity) from the bidder into escrow and gives the previous high bidder their money back. A bid at or above the buyout price buys the whole stack at the buyout price instead. |
| Buy out | Player | Buys a quantity at the buyout price: any quantity up to what is left for a commodity, the whole stack otherwise. The buyer pays, receives the items, the seller is paid. A previous high bidder gets their money back. |
| Cancel | Seller | Possible only while there is no bid. What is left of the stack goes back to the seller, and the deposit still held is lost. |
| Settle | Game host | Closes up to 100 listings whose end time has passed, per run. With a high bid: the items go to the high bidder and the seller is paid. Without: what is left goes back to the seller and the deposit still held is lost. The rest is closed on the next runs. |
| Search | Player | Lists the Active listings of one house (section 8). |

## 6. Business rules

1. The seller can neither bid on nor buy their own listing.
2. The high bidder cannot outbid themselves. They can still buy out, or bid at or above the buyout price.
3. A bid at or above the buyout price buys the whole stack at the buyout price, not at the bid price.
4. Buyout is possible while an auction is running. The high bidder gets their money back.
5. The first bid must be at least the start price. Each next bid must reach `high bid + max(absolute increment, high bid * increment rate)`, per unit.
6. A bid takes the total of the stack from the bidder (unit price times quantity) and holds it until the bidder is outbid or the listing ends.
7. At the end time without a bid, what is left of the stack goes back to the seller and the deposit still held is lost.
8. At the end time with a bid, the stack goes to the high bidder at their bid. The seller receives `total - commission + deposit`.
9. On a buyout, the seller receives `unit price * quantity bought - commission + deposit share`. The deposit share is `deposit * quantity bought / quantity left`, rounded down. Buying the last part gives back all the deposit still held, so nothing is lost to rounding. The listing keeps the deposit that covers what is left.
10. Only the seller can cancel, and only while there is no bid.
11. The commission is computed on the total of each sale: the whole stack for an auction, each part for a commodity. It uses the rate copied at creation.
12. The deposit is fixed at creation: `base value * quantity * deposit rate`, rounded down, then `* duration multiplier`. A deposit that rounds down to 0 is not taken. Example: 1 copper ore (base value 5 copper) at 15 % gives 0.75, so 0.
13. Commissions and lost deposits go to the sink. The game decides what happens to them.
14. Once its end time has passed, a listing accepts no bid, buyout or cancellation, even before it is settled.
15. A commodity refuses bids and needs a buyout price at creation. An auctioned item refuses a partial buyout.
16. At creation: quantity above 0, start price above 0, buyout price at least the start price, duration allowed by the house, and totals and deposit within the range of amounts.
17. For a commodity, the start price still has to be above 0 and at most the buyout price. The `game` interface sets it equal to the buyout price.
18. Every rate computation rounds down.

## 7. Money flows

### Commodity bought in part

House "Alliance" (commission 5 %, deposit 15 %). Alice sells 20 linen cloth (base value 13 copper) at 30 copper each, for 10 minutes (deposit multiplier 2).

| Step | Alice (seller) | Buyer | Sink | Listing |
|---|---|---|---|---|
| Sell | pays deposit `13 * 20 * 15 % = 39`, `* 2 = 78` | | | 20 left, deposit 78 |
| Bob buys 5 | receives `150 - 7 + 19 = 162` | Bob pays `5 * 30 = 150` | commission `150 * 5 % = 7.5`, so 7 | 15 left, deposit `78 - 19 = 59` (share `78 * 5 / 20 = 19.5`, so 19) |
| Carol buys 15 | receives `450 - 22 + 59 = 487` | Carol pays 450 | commission 22 | Sold, buyer Carol, 30 per unit |

Alice paid a deposit of 78 and got all 78 back. The sink received 29.

If the listing had ended after Bob's purchase instead, the 15 cloths left would go back to Alice, the 59 of deposit still held would go to the sink, and the listing would end Expired.

### Auctioned item

Same house. Alice sells 1 iron sword (base value 4,500 copper), start price 10,000, buyout price 50,000, for 2 minutes (multiplier 1).

| Step | Effect |
|---|---|
| Sell | Alice pays a deposit of `4,500 * 15 % = 675`. |
| Bob bids 10,000 | Bob pays 10,000 into escrow. The next bid must reach `10,000 + max(1, 10,000 * 5 %) = 10,500`. |
| Carol bids 10,500 | Carol pays 10,500 into escrow, Bob gets his 10,000 back. |
| Settlement | Carol receives the sword. Alice receives `10,500 - 525 + 675 = 10,650`. The sink receives 525. |

## 8. Search

A search returns the Active listings of one house, including the listings whose end time has passed but which are not settled yet. Optional filters: item type, seller, current high bidder. Results are sorted by unit price, lowest first, then by end time. The price used is the buyout price if there is one, otherwise the high bid or the start price. Results come in pages of 1 to 100 listings.

## 9. Errors

Messages returned to the caller when a request is refused.

| Message | When |
|---|---|
| `invalid auction house config: ...` | Opening a house whose terms break section 3. |
| `auction house not found` | Unknown house. |
| `listing not found` | Unknown listing. |
| `quantity must be greater than zero` | Selling 0 items. |
| `start price must be greater than zero` | Start price of 0. |
| `buyout price is below start price` | Buyout price lower than the start price. |
| `a commodity listing needs a buyout price` | Selling a commodity without a buyout price. |
| `duration not allowed by this auction house` | Duration not in the list of the house. |
| `amount overflow` | A total or a deposit is too large. |
| `item unavailable` | The seller does not hold the items. |
| `insufficient funds` | The player cannot pay the deposit, the bid or the buyout. |
| `listing is not active` | The listing is Sold, Expired or Cancelled. |
| `listing has expired` | The end time has passed, settlement pending. |
| `seller cannot bid on or buy their own listing` | Rule 1. |
| `already the highest bidder` | Rule 2. |
| `bid too low, minimum unit price is N` | Rule 5. |
| `a commodity cannot receive bids` | Bid on a commodity. |
| `listing has no buyout price` | Buyout of a listing without buyout price. |
| `quantity must be between 1 and N` | Buyout of a commodity with 0 or more than what is left. |
| `this item can only be bought as a whole, quantity must be N` | Partial buyout of an auctioned item. |
| `only the seller can cancel this listing` | Rule 10. |
| `listing has bids and cannot be cancelled` | Rule 10. |
| `page limit must be between 1 and 100` | Search page size out of range. |
| `listing changed concurrently, try again` | Another request changed the listing at the same time, three times in a row. |
| `unknown item type X` / `unknown card X` | Selling an item type or card that is not in the game catalog. |

## 10. Demo games

Two demo games trade on Agora. Both keep everything in memory, and their state is lost when the server restarts. The interface has no login: the player is picked from a list. The interface text is in French.

### 10.1 `game`

- Players hold gold and stacks of items. Amounts are in copper: 1 gold = 100 silver = 10,000 copper.
- The catalog lists the item types, their base value and whether they are commodities. Default catalog: linen cloth (13), copper ore (5) and healing potion (250) are commodities, the iron sword (4,500) is auctioned.
- Two houses by default, "Alliance" (commission 5 %) and "Neutre" (commission 15 %). Both use a deposit of 15 %, an increment of 5 % or at least 1 copper, and durations of 2, 10 and 30 minutes with deposit multipliers 1, 2 and 4.
- Three players, `alice`, `bob` and `carol`, start with 50 to 200 gold and 1 to 3 random item types of the catalog. Each starting stack is worth 0.2 to 2 gold, at least one item.
- The money taken out of the game is shown in the header ("Or retiré du jeu").

### 10.2 `tcg`

- Collectors hold dust (the money, one unit per dust) and copies of cards. Copies of a card are interchangeable.
- A listing always holds one card, bought whole. Cards are never commodities, so every card is auctioned. To sell several copies, a collector creates one listing per copy.
- The base value of a card comes from its rarity: common 40, rare 100, epic 400, legendary 1,600 dust by default.
- One house by default, "Marché" (commission 5 %), with the same deposit, increment and durations as in `game`.
- Three collectors, `alice`, `bob` and `carol`, start with 1,000 to 5,000 dust and 3 to 6 random cards, 1 to 3 copies each.

### 10.3 Interface behaviour

- From the interface, a player can sell, bid, buy out and cancel, and watch listings get settled. In `game`, a player can buy part of a commodity.
- The interface refreshes every second.
- Tabs: all listings ("Toutes les ventes"), the player's listings ("Mes ventes"), and the listings where the player is the current high bidder ("Mes enchères"). A listing where the player was outbid is not in "Mes enchères".
- Prices can be typed per unit or for the whole stack ("Prix du lot"). A stack price is divided by the quantity and rounded to the nearest copper, so the total paid can differ slightly from the total typed.
- The sell form shows the deposit before the listing is created.
- A listing past its end time shows "terminée" until it is settled.
- The table shows the first 50 listings of a house. There is no next page.

## 11. Functional limits

- **No authentication**: any request can act as any player. The interface trusts the player picked in the list.
- **Item attributes are not kept**: Agora stores and returns item attributes as given, but the demo ledgers count items by item type only, and the demo games set none.
- **One card per listing** in `tcg`.
- **One buyer recorded per Sold listing**: after several partial buyouts, only the last buyer and price are recorded on the listing. Earlier buyers appear only in the ledger of each game.
