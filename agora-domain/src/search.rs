use serde::{Deserialize, Serialize};

use crate::{HouseId, ItemType, PlayerRef};

/// Search over the active listings of one house, sorted by unit price then end time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub house: HouseId,
    pub item_type: Option<ItemType>,
    pub seller: Option<PlayerRef>,
    pub high_bidder: Option<PlayerRef>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Number of matches before pagination.
    pub total: u64,
}
