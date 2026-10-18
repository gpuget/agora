//! HTTP requests and responses between the trading card game server and the ui.

use agora_domain::{HouseId, ItemType, Listing, ListingId, Money, Page, PlayerRef, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardEntry {
    pub card: ItemType,
    pub name: String,
    pub rarity: Rarity,
    pub base_value: Money,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardCount {
    pub card: ItemType,
    pub copies: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectorView {
    pub collector: PlayerRef,
    pub dust: Money,
    pub cards: Vec<CardCount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationView {
    pub secs: u64,
    pub deposit_multiplier: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseView {
    pub id: HouseId,
    pub name: String,
    /// Basis points.
    pub commission: u32,
    /// Basis points of the card base value, before the duration multiplier.
    pub deposit: u32,
    pub durations: Vec<DurationView>,
}

/// Query string of `GET /api/houses/{id}/listings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListingsParams {
    pub card: Option<ItemType>,
    pub seller: Option<PlayerRef>,
    pub high_bidder: Option<PlayerRef>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Listings hold a single card: their unit prices are the prices of that card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingsResponse {
    /// Server time, to display the time left.
    pub now: Timestamp,
    pub page: Page<Listing>,
}

/// Puts one copy of a card up for sale. The server fills the base value from its catalog:
/// the client never sends it. Unknown fields such as `quantity` are rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateListingRequest {
    pub house: HouseId,
    pub seller: PlayerRef,
    pub card: ItemType,
    pub start_price: Money,
    pub buyout_price: Option<Money>,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingResponse {
    pub id: ListingId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidRequest {
    pub bidder: PlayerRef,
    pub price: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BidResult {
    HighBidder,
    Bought,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidResponse {
    pub result: BidResult,
}

/// Body of buyout and cancel requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectorRequest {
    pub collector: PlayerRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}
