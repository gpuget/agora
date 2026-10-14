//! Types and pure rules of the auction domain. No I/O, no transport.

mod error;
mod house;
mod ids;
mod item;
mod listing;
mod money;
mod search;
mod time;

pub use error::{AuctionError, PortError};
pub use house::{AuctionHouse, DurationOption, HouseConfig};
pub use ids::{HouseId, ItemType, ListingId, OpId, PlayerRef};
pub use item::ItemStack;
pub use listing::{Bid, Listing, ListingStatus};
pub use money::{Bps, Money};
pub use search::{Page, SearchQuery};
pub use time::Timestamp;
