# 0011. One card per listing in the trading card game

- Status: Accepted
- Date: 2026-10-18

## Context

The trading card game trades copies of cards. Copies of a card are interchangeable.

## Decision

A `tcg` listing always holds a single copy, bought whole. `SellCommand` and the HTTP request have no quantity, and the request rejects unknown fields. Prices are those of the card. Cards are never commodities ([0012](0012-commodities-bought-in-part.md)), so every card is auctioned.

## Consequences

- No quantity to enter or to validate in `tcg`, and no difference between unit and total price.
- A collector who wants to sell several copies creates one listing per copy.
