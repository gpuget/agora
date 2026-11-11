//! Types and pure rules of the auction domain. No I/O, no transport.
//!
//! Center of the Agora hexagon. `agora-api` (inbound ports), `agora-spi` (outbound ports) and
//! `agora-core` (use cases) are written in these types, and so are the adapters that drive or
//! implement those ports.
//!
//! Conventions shared by every crate of Agora:
//! - [`Money`] is an amount in the smallest unit of the game's currency.
//! - A [`Bps`] is a rate in basis points: `500` is 5 %. Applying a rate rounds down.
//! - Prices are per unit. The total of a stack is the unit price times the quantity.
//! - A [`Timestamp`] counts milliseconds since the Unix epoch.
//!
//! Business rules: see `docs/functional.md`.

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
