# 0003. The game server is the only, trusted client of Agora

- Status: Accepted
- Date: 2026-10-14

## Context

A listing needs facts that only the game knows: who the player is, the base value of an item (used for the deposit) and whether it is a commodity.

## Decision

Players never call Agora. The game server calls it on their behalf and is trusted: Agora takes the `PlayerRef`, the base value and the commodity flag as given.

The demo games read the base value and the commodity flag from their own catalog (`GameService::sell`, `TcgService::sell`), never from the HTTP request.

## Consequences

- Agora has no authentication and no notion of user.
- Any adapter exposed to players must fill these values on the server side. A client able to set the base value could lower its deposit.
- The demo servers do not authenticate players either: their HTTP API accepts any `PlayerRef` ([roadmap](../roadmap.md)).
