//! Domain of the demo trading card game: collectors, their dust and cards, and the card
//! catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `ItemType`) since the game trades on it. Dust is the
//! money of the game: one [`Money`] unit is one dust. The dust and cards of a collector are an
//! [`Account`] of the ledger, with the dust held as its funds and the cards as its items.

use std::fmt;

use agora_domain::{AuctionError, ItemType, Money};
use event_sourcing::StoreError;
use ledger::Account;
use thiserror::Error;

/// Identifies a card of the catalog. Copies of a card are interchangeable.
pub type CardId = ItemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

impl fmt::Display for Rarity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Rarity::Common => "common",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEntry {
    pub card: CardId,
    pub name: String,
    pub rarity: Rarity,
    /// Value of one copy, set by its rarity. The game is the only source of this value.
    pub base_value: Money,
}

pub type Collector = Account;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TcgError {
    #[error("unknown card {0}")]
    UnknownCard(CardId),
    #[error(transparent)]
    Auction(#[from] AuctionError),
    /// The event stream changed since it was loaded.
    #[error("concurrent change, retry")]
    Conflict,
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<StoreError> for TcgError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Conflict => TcgError::Conflict,
            StoreError::Storage(message) => TcgError::Storage(message),
        }
    }
}
