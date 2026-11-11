//! HTTP requests and responses between the demo game server and the ui.
//!
//! # Routes
//!
//! Every body is JSON.
//!
//! | Method | Path | Request | Success |
//! |---|---|---|---|
//! | `GET` | `/api/catalog` | | `200`, array of [`CatalogEntry`] |
//! | `GET` | `/api/houses` | | `200`, array of [`HouseView`] |
//! | `GET` | `/api/players` | | `200`, array of [`PlayerView`] |
//! | `GET` | `/api/sink` | | `200`, [`Money`]: money taken out of the game |
//! | `GET` | `/api/houses/{house}/listings` | query [`ListingsParams`] | `200`, [`ListingsResponse`] |
//! | `POST` | `/api/listings` | [`CreateListingRequest`] | `200`, [`CreateListingResponse`] |
//! | `POST` | `/api/listings/{id}/bid` | [`BidRequest`] | `200`, [`BidResponse`] |
//! | `POST` | `/api/listings/{id}/buyout` | [`BuyoutRequest`] | `204`, no body |
//! | `POST` | `/api/listings/{id}/cancel` | [`PlayerRequest`] | `204`, no body |
//!
//! `{house}` and `{id}` are UUIDs. `/api/players` and `/api/sink` reflect every ledger call
//! completed before the request.
//!
//! # Units
//!
//! Every [`Money`] is a JSON integer in copper: 1 gold = 100 silver = 10,000 copper. Prices
//! are per unit: the total of a stack is unit price * quantity. [`Timestamp`]s are
//! milliseconds since the Unix epoch.
//!
//! # Errors
//!
//! A rejected call answers with an [`ErrorBody`] and one of these statuses:
//!
//! - `404`: house or listing not found.
//! - `409`: concurrent change, retry.
//! - `422`: refused by a business rule, such as a bid too low, an unknown item type,
//!   insufficient funds or a page `limit` out of range.
//! - `500`: storage failure, or any ledger failure other than insufficient funds or item
//!   unavailable. A player without an account falls here.
//!
//! A malformed request is refused by axum before the handler, with a plain text body instead
//! of an [`ErrorBody`]: `400` for bad JSON, a bad path UUID or a bad query string, `415`
//! without `Content-Type: application/json`, `422` for JSON of the wrong shape.

use agora_domain::{HouseId, ItemType, Listing, ListingId, Money, Page, PlayerRef, Timestamp};
use serde::{Deserialize, Serialize};

/// Element of `GET /api/catalog`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub item_type: ItemType,
    /// Display name.
    pub name: String,
    /// Value of one item, in copper. Agora computes the deposit from it.
    pub base_value: Money,
    /// `true` for a commodity: fixed price only, can be bought in part. Other items are
    /// auctioned and bought whole.
    pub commodity: bool,
}

/// Items of one item type held by a player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemCount {
    pub item_type: ItemType,
    /// Always greater than zero.
    pub quantity: u32,
}

/// Element of `GET /api/players`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub player: PlayerRef,
    /// Funds of the player, in copper. Excludes the money held in escrow by Agora.
    pub gold: Money,
    /// Sorted by item type. Excludes the stacks held in escrow by Agora.
    pub items: Vec<ItemCount>,
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
    /// Of the total price of a sale, taken from the seller's proceeds. 10,000 is 100%.
    pub commission: u32,
    /// Basis points of the stack base value, before the duration multiplier.
    ///
    /// The stack base value is base value * quantity. The seller pays the deposit when the
    /// listing is created.
    pub deposit: u32,
    /// The only durations a listing of this house can have.
    pub durations: Vec<DurationView>,
}

/// Query string of `GET /api/houses/{id}/listings`.
///
/// Every filter is optional. An unknown house gives an empty page, not a `404`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListingsParams {
    /// Only listings of this item type.
    pub item_type: Option<ItemType>,
    /// Only listings of this seller.
    pub seller: Option<PlayerRef>,
    /// Only listings where this player holds the high bid.
    pub high_bidder: Option<PlayerRef>,
    /// Page size, 50 if absent. Agora answers `422` below 1 or above 100.
    pub limit: Option<u32>,
    /// Number of matches skipped, 0 if absent.
    pub offset: Option<u32>,
}

/// Response of `GET /api/houses/{house}/listings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingsResponse {
    /// Server time, to display the time left.
    pub now: Timestamp,
    /// Listings not settled yet, sorted by unit price (buyout price if any, else the high
    /// bid or the start price), then end time. May include listings past their end time
    /// that the next settlement will close. `total` counts the matches before pagination.
    pub page: Page<Listing>,
}

/// The server fills the base value from its catalog: the client never sends it.
///
/// Body of `POST /api/listings`. Agora takes the stack from the seller into escrow and
/// debits the deposit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingRequest {
    pub house: HouseId,
    pub seller: PlayerRef,
    /// Must be in the catalog.
    pub item_type: ItemType,
    /// Number of items in the stack, at least 1.
    pub quantity: u32,
    /// Per unit, in copper, at least 1.
    pub start_unit_price: Money,
    /// Per unit, in copper, not below the start price. Required for a commodity.
    pub buyout_unit_price: Option<Money>,
    /// One of the [`DurationView::secs`] of the house.
    pub duration_secs: u64,
}

/// Response of `POST /api/listings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingResponse {
    /// The new listing.
    pub id: ListingId,
}

/// Body of `POST /api/listings/{id}/bid`. A commodity takes no bids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidRequest {
    pub bidder: PlayerRef,
    /// Per unit, in copper. Agora holds unit price * stack quantity in escrow until
    /// the bid is outbid or settled. At or above the buyout price, the whole stack is bought
    /// at the buyout price.
    pub unit_price: Money,
}

/// Outcome of a bid, serialized as `"high_bidder"` or `"bought"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BidResult {
    /// The bid is now the high bid.
    HighBidder,
    /// The bid reached the buyout price: the bidder bought the whole stack at the buyout
    /// price.
    Bought,
}

/// Response of `POST /api/listings/{id}/bid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidResponse {
    pub result: BidResult,
}

/// Body of cancel requests.
///
/// `POST /api/listings/{id}/cancel`. Only the seller can cancel, and only a listing without
/// bids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerRequest {
    /// The seller.
    pub player: PlayerRef,
}

/// Body of buyout requests. `quantity` is the number of items bought.
///
/// `POST /api/listings/{id}/buyout`. The buyer pays buyout price * `quantity`. Only a
/// commodity can be bought in part: for other items, `quantity` must be the whole stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuyoutRequest {
    /// The buyer.
    pub player: PlayerRef,
    pub quantity: u32,
}

/// Body of every error answered by the handlers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
}
