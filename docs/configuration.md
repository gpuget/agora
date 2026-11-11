# Configuration

For whoever runs a demo server and wants to change its settings. This page explains the keys and their rules. The values each game uses are in its settings file, [game.toml](../game.toml) or [tcg.toml](../tcg.toml): each file shows every default with a comment, and a test (`shipped_file_shows_the_defaults`) checks that it matches the defaults in the code.

## Where settings come from

From lowest to highest priority:

1. The defaults in the code (`game/server/src/settings.rs`, `tcg/server/src/settings.rs`).
2. The settings file: `game.toml` or `tcg.toml` in the working directory, or the file named by `GAME_CONFIG` or `TCG_CONFIG`. A missing file means defaults.
3. Environment variables: `BIND_ADDR` replaces `bind_addr`, `RUST_LOG` replaces the whole `[log]` table.

The working directory matters: start the servers from the repository root, or set `GAME_CONFIG` / `TCG_CONFIG`. Elsewhere they find no file, have no catalog and refuse to start.

## Keys

Every top-level key is optional except `catalog`. Inside a `[[house]]` table, a catalog entry or the `rarity` table, every key is required, except `commodity` in the `game` catalog. Unknown keys are rejected.

| Key | Type | Rules |
|---|---|---|
| `bind_addr` | string, `host:port` | Listen address of the HTTP API. |
| `settle_every_secs` | integer | Seconds between two settlements of expired listings. Above zero. |
| `[log]` | table, target = level | Level per log target: `trace`, `debug`, `info`, `warn`, `error` or `off`. The key `default` sets every target not listed. |
| `[[house]]` | list of tables | Houses opened at startup. At least one. Writing one replaces all default houses. |
| `house.name` | string | Unique among houses. |
| `house.commission` | integer, bps | Taken from the total of each sale. At most 10,000 (100 %). |
| `house.min_increment` | integer, money | Absolute part of the minimum bid increment, per unit. At least 1. |
| `house.increment` | integer, bps | Relative part of the minimum bid increment. |
| `house.deposit` | integer, bps | Deposit rate, applied to the base value of the stack. |
| `house.durations` | list of `{ minutes, deposit_multiplier }` | At least one. Each duration above zero and listed once, each multiplier above zero. |
| `catalog` (`game`) | list of `{ item_type, name, base_value, commodity }` | At least one entry. `item_type` unique, `base_value` above zero (in copper), `commodity` optional and `false` by default. |
| `catalog` (`tcg`) | list of `{ card_id, name, rarity }` | At least one entry. `card_id` unique, `rarity` is `common`, `rare`, `epic` or `legendary`. |
| `rarity` (`tcg`) | `{ common, rare, epic, legendary }` | Value of one card in dust per rarity. Optional; when present, every rarity is required and above zero. |

Rates are in basis points: 100 = 1 %. What the house terms mean for a sale is explained in [functional.md](functional.md#3-auction-houses).

Write `catalog` above `[log]` and `[[house]]`: in TOML, a key written after a table header belongs to that table.

## Who checks what

- The server checks the file at startup and refuses to start on the first error: unknown key, duplicate house name or catalog id, duration listed twice, zero multiplier, zero base value or rarity value, empty catalog, `settle_every_secs` of zero.
- Agora checks the house terms when the house is opened: commission at most 100 %, `min_increment` at least 1, at least one duration, no zero duration.

## Logs

Logs go to stdout. With the shipped log table they show business events, rejected requests, settlement and every ledger call Agora makes, with its `OpId`. Set `tower_http` to `debug` to also log every HTTP response (the interface polls every second, so this is verbose), or every level to `info` to keep only business events, rejected requests and settlement.

## Interface proxy

The interfaces do not read these files. `trunk serve` proxies `/api/` to the `backend` address written in `game/ui/Trunk.toml` or `tcg/ui/Trunk.toml`, and serves on the `port` written there. If you change `bind_addr` or `BIND_ADDR`, change the `backend` of the matching `Trunk.toml` too.
