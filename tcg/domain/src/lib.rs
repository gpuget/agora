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

/// Rarity of a card, from the most common to the rarest. Sets the base value of the card.
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

/// A card of the catalog. Only cards of the catalog can be sold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEntry {
    pub card: CardId,
    /// Display name.
    pub name: String,
    pub rarity: Rarity,
    /// Value of one copy, set by its rarity. The game is the only source of this value.
    ///
    /// In dust. Agora computes the deposit of a listing from it.
    pub base_value: Money,
}

/// A collector: its ledger [`Account`], dust as funds and copies held by card.
pub type Collector = Account;

/// Failure of a trading card game use case.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TcgError {
    /// The card is not in the catalog.
    #[error("unknown card {0}")]
    UnknownCard(CardId),
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

impl From<StoreError> for TcgError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Conflict => TcgError::Conflict,
            StoreError::Storage(message) => TcgError::Storage(message),
        }
    }
}
