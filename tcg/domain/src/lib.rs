//! Domain of the demo trading card game: collectors, their dust and cards, and the card
//! catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `PlayerRef`, `ItemType`) since the game trades on it.
//! Dust is the money of the game: one [`Money`] unit is one dust.

use std::collections::BTreeMap;
use std::fmt;

use agora_domain::{AuctionError, ItemType, ListingId, Money, PlayerRef};
use event_sourcing::StoreError;
use thiserror::Error;

mod ledger;

pub use ledger::{
    CollectorCommand, CollectorDecider, CollectorEvent, CollectorState, TreasuryCommand,
    TreasuryDecider, TreasuryEvent, TreasuryId, TreasuryState,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collector {
    pub id: PlayerRef,
    pub dust: Money,
    /// Only cards held in at least one copy.
    pub cards: BTreeMap<CardId, u32>,
}

impl Collector {
    pub fn new(id: PlayerRef, dust: Money) -> Self {
        Self {
            id,
            dust,
            cards: BTreeMap::new(),
        }
    }

    pub fn with_card(mut self, card: CardId, copies: u32) -> Self {
        if copies > 0 {
            self.cards.insert(card, copies);
        }
        self
    }

    pub fn copies(&self, card: &CardId) -> u32 {
        self.cards.get(card).copied().unwrap_or(0)
    }

    pub fn check_withdraw(&self, card: &CardId, copies: u32) -> Result<(), TcgError> {
        if self.copies(card) < copies {
            return Err(TcgError::CardUnavailable);
        }
        Ok(())
    }

    pub fn check_deposit(&self, card: &CardId, copies: u32) -> Result<(), TcgError> {
        self.copies(card)
            .checked_add(copies)
            .ok_or(TcgError::Overflow)?;
        Ok(())
    }

    pub fn check_debit(&self, amount: Money) -> Result<(), TcgError> {
        self.dust
            .checked_sub(amount)
            .ok_or(TcgError::InsufficientDust)?;
        Ok(())
    }

    pub fn check_credit(&self, amount: Money) -> Result<(), TcgError> {
        self.dust.checked_add(amount).ok_or(TcgError::Overflow)?;
        Ok(())
    }
}

/// Why the auction house took dust out of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakenReason {
    Commission { listing: ListingId },
    ForfeitedDeposit { listing: ListingId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TcgError {
    #[error("unknown collector {0}")]
    UnknownCollector(PlayerRef),
    #[error("collector {0} already exists")]
    CollectorExists(PlayerRef),
    #[error("unknown card {0}")]
    UnknownCard(CardId),
    #[error("card unavailable")]
    CardUnavailable,
    #[error("insufficient dust")]
    InsufficientDust,
    #[error("amount overflow")]
    Overflow,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn drake() -> CardId {
        "fire_drake".into()
    }

    fn alice() -> Collector {
        Collector::new("alice".into(), Money(100)).with_card(drake(), 2)
    }

    #[test]
    fn withdraw_needs_the_copies() {
        let collector = alice();
        assert_eq!(collector.check_withdraw(&drake(), 2), Ok(()));
        assert_eq!(
            collector.check_withdraw(&drake(), 3),
            Err(TcgError::CardUnavailable)
        );
        assert_eq!(
            collector.check_withdraw(&"goblin_scout".into(), 1),
            Err(TcgError::CardUnavailable)
        );
    }

    #[test]
    fn debit_needs_the_dust() {
        let collector = alice();
        assert_eq!(collector.check_debit(Money(100)), Ok(()));
        assert_eq!(
            collector.check_debit(Money(101)),
            Err(TcgError::InsufficientDust)
        );
    }

    #[test]
    fn credit_and_deposit_detect_overflow() {
        let collector = alice();
        assert_eq!(
            collector.check_credit(Money(u64::MAX)),
            Err(TcgError::Overflow)
        );
        assert_eq!(
            collector.check_deposit(&drake(), u32::MAX),
            Err(TcgError::Overflow)
        );
    }
}
