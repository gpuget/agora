//! Domain of the demo game: players, their gold and items, and the item catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `ItemType`) since the game trades on it. The gold and
//! items of a player are an [`Account`] of the ledger, with the gold held as its funds.
//!
//! Every [`Money`] amount of the game is in copper: 1 gold = 100 silver = 10,000 copper.

use agora_domain::{AuctionError, ItemType, Money};
use event_sourcing::StoreError;
use ledger::Account;
use thiserror::Error;

/// An item type of the catalog. Only item types of the catalog can be sold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub item_type: ItemType,
    /// Display name.
    pub name: String,
    /// Value of one item. The game is the only source of this value.
    ///
    /// In copper. Agora computes the deposit of a listing from it.
    pub base_value: Money,
    /// Bought in part and never auctioned, as opposed to equipment and rare items.
    pub commodity: bool,
}

/// A player: its ledger [`Account`], gold as funds (in copper) and items by item type.
pub type Player = Account;

/// Failure of a game use case.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GameError {
    /// The item type is not in the catalog.
    #[error("unknown item type {0}")]
    UnknownItemType(ItemType),
    /// Rejected or failed in Agora. Ledger failures reach the game through this variant, as
    /// [`AuctionError::Port`].
    #[error(transparent)]
    Auction(#[from] AuctionError),
    /// The event stream changed since it was loaded.
    #[error("concurrent change, retry")]
    Conflict,
    /// The event store failed.
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<StoreError> for GameError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Conflict => GameError::Conflict,
            StoreError::Storage(message) => GameError::Storage(message),
        }
    }
}
