use serde::{Deserialize, Serialize};

use crate::{HouseId, ItemType, PlayerRef};

/// Search over the active listings of one house, sorted by unit price then end time.
///
/// The unit price used to sort is [`Listing::sort_unit_price`](crate::Listing::sort_unit_price).
/// Every filter set must match. A filter left to `None` matches every listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// House to search in.
    pub house: HouseId,
    /// Keeps only the listings of this item type.
    pub item_type: Option<ItemType>,
    /// Keeps only the listings of this seller.
    pub seller: Option<PlayerRef>,
    /// Keeps only the listings whose high bid belongs to this player.
    pub high_bidder: Option<PlayerRef>,
    /// Maximum number of listings returned. The core accepts 1 to 100 and fails with
    /// [`AuctionError::InvalidPage`](crate::AuctionError::InvalidPage) otherwise.
    pub limit: u32,
    /// Number of sorted matches skipped before the page starts.
    pub offset: u32,
}

/// One page of search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    /// Results of the page, in sort order.
    pub items: Vec<T>,
    /// Number of matches before pagination.
    pub total: u64,
}
