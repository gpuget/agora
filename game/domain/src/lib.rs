//! Domain of the demo game: players, their gold and items, and the item catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `ItemType`) since the game trades on it. The gold and
//! items of a player are an [`Account`] of the ledger, with the gold held as its funds.

use agora_domain::{AuctionError, ItemType, Money};
use event_sourcing::StoreError;
use ledger::Account;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub item_type: ItemType,
    pub name: String,
    /// Value of one item. The game is the only source of this value.
    pub base_value: Money,
}

pub type Player = Account;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GameError {
    #[error("unknown item type {0}")]
    UnknownItemType(ItemType),
    #[error(transparent)]
    Auction(#[from] AuctionError),
    /// The event stream changed since it was loaded.
    #[error("concurrent change, retry")]
    Conflict,
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
