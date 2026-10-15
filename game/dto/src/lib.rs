//! HTTP requests and responses between the demo game server and the ui.

use agora_domain::{HouseId, ItemType, Listing, ListingId, Money, Page, PlayerRef, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub item_type: ItemType,
    pub name: String,
    pub base_value: Money,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemCount {
    pub item_type: ItemType,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub player: PlayerRef,
    pub gold: Money,
    pub items: Vec<ItemCount>,
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
    /// Basis points of the stack base value, before the duration multiplier.
    pub deposit: u32,
    pub durations: Vec<DurationView>,
}

/// Query string of `GET /api/houses/{id}/listings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListingsParams {
    pub item_type: Option<ItemType>,
    pub seller: Option<PlayerRef>,
    pub high_bidder: Option<PlayerRef>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListingsResponse {
    /// Server time, to display the time left.
    pub now: Timestamp,
    pub page: Page<Listing>,
}

/// The server fills the base value from its catalog: the client never sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingRequest {
    pub house: HouseId,
    pub seller: PlayerRef,
    pub item_type: ItemType,
    pub quantity: u32,
    pub start_unit_price: Money,
    pub buyout_unit_price: Option<Money>,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateListingResponse {
    pub id: ListingId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidRequest {
    pub bidder: PlayerRef,
    pub unit_price: Money,
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
pub struct PlayerRequest {
    pub player: PlayerRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}
