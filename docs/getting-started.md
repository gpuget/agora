# Getting started

A first tour for a developer new to Agora: install the tools, run the demo game, then sell, buy and bid, and watch a sale get settled. It takes about 15 minutes, most of it waiting for builds and for an auction to end.

The values quoted below (houses, items, rates) are those of the shipped `game.toml`.

## 1. Install the tools

- Rust via [rustup](https://rustup.rs). The workspace uses edition 2024 and let chains, so Rust 1.88 at least; it is developed with Rust 1.99.
- The WebAssembly target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev): `cargo install --locked trunk`, or a prebuilt binary from the Trunk releases page.
- Windows only: MSVC Build Tools with the "Desktop development with C++" workload. Without them `link.exe` is missing and `cargo build` fails.

Check the setup from the repository root:

```
cargo test --workspace
```

## 2. Start the game

Every command runs from the repository root, because the server reads `game.toml` from the working directory.

Terminal 1, the server:

```
cargo run -p game-server
```

Wait for `listening on http://127.0.0.1:8080`. The log above it lists the three players, `alice`, `bob` and `carol`, with their gold and items. Items are drawn at random at each start.

Terminal 2, the interface:

```
cd game/ui
trunk serve
```

The first build takes a while. Then open <http://127.0.0.1:3000>.

## 3. Find your way

- Header: "Joueur" picks the player you act as (there is no login), "Hôtel des ventes" picks the house, "Or retiré du jeu" shows the money the houses took out of the game.
- Left: "Mon personnage" shows your gold and items, and "Vendre" is the sell form. Amounts read `po` (gold), `pa` (silver), `pc` (copper): 1 po = 100 pa = 10,000 pc.
- Right: the listings of the house. "Toutes les ventes" shows all of them, "Mes ventes" yours, "Mes enchères" those where you hold the high bid.

The page refreshes every second.

## 4. Sell a commodity

Linen cloth, copper ore and healing potions are commodities: sold at a fixed price, bought in any quantity.

1. In "Joueur", pick a player who holds one of them ("Étoffe de lin", "Minerai de cuivre" or "Potion de soins"). Try the others if needed.
2. In "Vendre", choose the item, set "Quantité" to 10, keep "Prix unitaire", and type 100 in "Prix / u (pc)".
3. Pick the 10-minute duration. The form shows the deposit it will cost.
4. Click "Mettre en vente".

Your items and your gold go down: the stack is now held by the house, and you paid the deposit. The listing shows in "Mes ventes".

## 5. Buy part of it

1. Switch "Joueur" to another player.
2. On the listing, type 3 in the quantity field and click "Acheter".

The buyer pays 300 pc and receives 3 items. Switch back to the seller: they received 300 pc minus the commission of the house, plus the share of the deposit that covers 3 items. The listing stays open with 7 items, and "Or retiré du jeu" went up by the commission.

How each amount is computed: [functional.md, section 7](functional.md#7-money-flows).

## 6. Run an auction

The iron sword ("Épée en fer") is not a commodity: it is auctioned and bought whole.

1. Pick a player who holds one. If nobody does, restart the server: items are drawn again.
2. Sell it with a start price ("Mise à prix / u (pc)") of 10000, no buyout price, and the 2-minute duration.
3. Switch to a second player. The bid field of the listing holds the start price: click "Enchérir".
4. Switch to a third player, type 11000 in the bid field and click "Enchérir". A new bid must add at least 5 % to the previous one, so 10500 at least here. The second player gets their money back, and the listing leaves their "Mes enchères" tab.

## 7. Watch the settlement

When "Temps restant" reaches "terminée", the auction is over: it refuses any bid but has not paid anyone yet. Within a second the server settles it and the listing disappears. The last bidder holds the sword, and the seller received the bid minus the commission, plus the whole deposit.

The commodity listing from step 4 ends the same way after 10 minutes. Since nobody bought the last 7 items, they go back to the seller, and the deposit still held goes to "Or retiré du jeu".

## 8. Try the trading card game

Stop both terminals, or keep them running: the card game uses other ports.

```
cargo run -p tcg-server
```

```
cd tcg/ui
trunk serve
```

Open <http://127.0.0.1:3001>. Collectors hold dust and cards. A listing always holds one card, and every card is auctioned.

## Next

- The full rules: [functional.md](functional.md).
- Plug Agora into your own game: [integrate-a-game.md](integrate-a-game.md).
- Change the houses, the catalog or the ports: [configuration.md](configuration.md).
- How the code is organised: [architecture.md](architecture.md).
