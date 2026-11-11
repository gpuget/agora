//! HTTP requests and responses between the trading card game server and the ui.
//!
//! # Routes
//!
//! Every body is JSON.
//!
//! | Method | Path | Request | Success |
//! |---|---|---|---|
//! | `GET` | `/api/catalog` | | `200`, array of [`CardEntry`] |
//! | `GET` | `/api/houses` | | `200`, array of [`HouseView`] |
//! | `GET` | `/api/collectors` | | `200`, array of [`CollectorView`] |
//! | `GET` | `/api/sink` | | `200`, [`Money`]: dust taken out of the game |
//! | `GET` | `/api/houses/{house}/listings` | query [`ListingsParams`] | `200`, [`ListingsResponse`] |
//! | `POST` | `/api/listings` | [`CreateListingRequest`] | `200`, [`CreateListingResponse`] |
//! | `POST` | `/api/listings/{id}/bid` | [`BidRequest`] | `200`, [`BidResponse`] |
//! | `POST` | `/api/listings/{id}/buyout` | [`CollectorRequest`] | `204`, no body |
//! | `POST` | `/api/listings/{id}/cancel` | [`CollectorRequest`] | `204`, no body |
//!
//! `{house}` and `{id}` are UUIDs. `/api/collectors` and `/api/sink` reflect every ledger
//! call completed before the request.
//!
//! # Differences with the game API
//!
//! A listing always holds one copy of a card. The requests carry no quantity, and their
//! price fields drop the `unit_` prefix (`start_price`, `buyout_price`, `price`). The
//! [`Listing`]s returned by the search keep the Agora names (`start_unit_price`,
//! `buyout_unit_price`, `high_bid.unit_price`) with `item.quantity` always 1. The buyout and
//! cancel routes share [`CollectorRequest`].
//!
//! # Units
//!
//! Every [`Money`] is a JSON integer in dust. A price is the price of the one card, so it is
//! also the total. [`Timestamp`]s are milliseconds since the Unix epoch.
//!
//! # Errors
//!
//! A rejected call answers with an [`ErrorBody`] and one of these statuses:
//!
//! - `404`: house or listing not found.
//! - `409`: concurrent change, retry.
//! - `422`: refused by a business rule, such as a bid too low, an unknown card, insufficient
//!   dust or a page `limit` out of range.
//! - `500`: storage failure, or any ledger failure other than insufficient funds or item
//!   unavailable. A collector without an account falls here.
//!
//! A malformed request is refused by axum before the handler, with a plain text body instead
//! of an [`ErrorBody`]: `400` for bad JSON, a bad path UUID or a bad query string, `415`
//! without `Content-Type: application/json`, `422` for JSON of the wrong shape.

use agora_domain::{HouseId, ItemType, Listing, ListingId, Money, Page, PlayerRef, Timestamp};
use serde::{Deserialize, Serialize};

/// Rarity of a card, serialized in lower case (`"common"`, `"rare"`, `"epic"`,
/// `"legendary"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

/// Element of `GET /api/catalog`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardEntry {
    pub card: ItemType,
    /// Display name.
    pub name: String,
    pub rarity: Rarity,
    /// Value of one copy, in dust, set by its rarity. Agora computes the deposit from it.
    pub base_value: Money,
}

/// Copies of one card held by a collector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardCount {
    pub card: ItemType,
    /// Always greater than zero.
    pub copies: u32,
}

/// Element of `GET /api/collectors`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectorView {
    pub collector: PlayerRef,
    /// Funds of the collector. Excludes the dust held in escrow by Agora.
    pub dust: Money,
    /// Sorted by card. Excludes the cards held in escrow by Agora.
    pub cards: Vec<CardCount>,
}

/// A listing duration allowed by a house.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationView {
    /// Value to send as [`CreateListingRequest::duration_secs`].
    pub secs: u64,
    /// Multiplies the deposit of a listing with this duration.
    pub deposit_multiplier: u32,
}

/// Element of `GET /api/houses`. The minimum increment is not exposed here: each
/// [`Listing`] carries its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseView {
    pub id: HouseId,
    pub name: String,
    /// Basis points.
    ///
    /// Of the price of a sale, taken from the seller's proceeds. 10,000 is 100%.
    pub commission: u32,
    /// Basis points of the card base value, before the duration multiplier.
    ///
    /// The seller pays the deposit when the listing is created.
    pub deposit: u32,
    /// The only durations a listing of this house can have.
    pub durations: Vec<DurationView>,
}

/// Query string of `GET /api/houses/{id}/listings`.
///
/// Every filter is optional. An unknown house gives an empty page, not a `404`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListingsParams {
    /// Only listings of this card. Named `item_type` in the game API.
    pub card: Option<ItemType>,
    /// Only listings of this seller.
    pub seller: Option<PlayerRef>,
    /// Only listings where this collector holds the high bid.
    pub high_bidder: Option<PlayerRef>,
    /// Page size, 50 if absent. Agora answers `422` below 1 or above 100.
    pub limit: Option<u32>,
    /// Number of matches skipped, 0 if absent.
    pub offset: Option<u32>,
}

/// Listings hold a single card: their unit prices are the prices of that card.
///
/// Response of `GET /api/houses/{house}/listings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingsResponse {
    /// Server time, to display the time left.
    pub now: Timestamp,
    /// Listings not settled yet, sorted by price (buyout price if any, else the high bid or
    /// the start price), then end time. May include listings past their end time that the
    /// next settlement will close. `total` counts the matches before pagination.
    pub page: Page<Listing>,
}

/// Puts one copy of a card up for sale. The server fills the base value from its catalog:
/// the client never sends it. Unknown fields such as `quantity` are rejected.
///
/// Body of `POST /api/listings`. An unknown field gets axum's plain text `422`. Agora takes
/// the card from the seller into escrow and debits the deposit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateListingRequest {
    pub house: HouseId,
    pub seller: PlayerRef,
    /// Must be in the catalog.
    pub card: ItemType,
    /// In dust, at least 1.
    pub start_price: Money,
    /// In dust, not below the start price.
    pub buyout_price: Option<Money>,
    /// One of the [`DurationView::secs`] of the house.
    pub duration_secs: u64,
}

/// Response of `POST /api/listings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingResponse {
    /// The new listing.
    pub id: ListingId,
}

/// Body of `POST /api/listings/{id}/bid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidRequest {
    pub bidder: PlayerRef,
    /// In dust. Agora holds it in escrow until the bid is outbid or settled. At or above the
    /// buyout price, the card is bought at the buyout price.
    pub price: Money,
}

/// Outcome of a bid, serialized as `"high_bidder"` or `"bought"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BidResult {
    /// The bid is now the high bid.
    HighBidder,
    /// The bid reached the buyout price: the bidder bought the card at the buyout price.
    Bought,
}

/// Response of `POST /api/listings/{id}/bid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidResponse {
    pub result: BidResult,
}

/// Body of buyout and cancel requests.
///
/// `POST /api/listings/{id}/buyout`: the collector buys the card at its buyout price.
/// `POST /api/listings/{id}/cancel`: the collector is the seller; only a listing without
/// bids can be cancelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectorRequest {
    pub collector: PlayerRef,
}

/// Body of every error answered by the handlers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
}
