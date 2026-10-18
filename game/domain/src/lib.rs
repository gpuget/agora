//! Domain of the demo game: players, their gold and items, and the item catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `PlayerRef`, `ItemType`) since the game trades on it.

use std::collections::BTreeMap;

use agora_domain::{AuctionError, ItemType, ListingId, Money, PlayerRef};
use thiserror::Error;

mod ledger;

pub use ledger::{
    Decider, PlayerCommand, PlayerDecider, PlayerEvent, PlayerState, TreasuryCommand,
    TreasuryDecider, TreasuryEvent, TreasuryId, TreasuryState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub item_type: ItemType,
    pub name: String,
    /// Value of one item. The game is the only source of this value.
    pub base_value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerRef,
    pub gold: Money,
    /// Only item types held in a quantity greater than zero.
    pub items: BTreeMap<ItemType, u32>,
}

impl Player {
    pub fn new(id: PlayerRef, gold: Money) -> Self {
        Self {
            id,
            gold,
            items: BTreeMap::new(),
        }
    }

    pub fn with_item(mut self, item_type: ItemType, quantity: u32) -> Self {
        if quantity > 0 {
            self.items.insert(item_type, quantity);
        }
        self
    }

    pub fn quantity(&self, item_type: &ItemType) -> u32 {
        self.items.get(item_type).copied().unwrap_or(0)
    }

    pub fn check_withdraw(&self, item_type: &ItemType, quantity: u32) -> Result<(), GameError> {
        if self.quantity(item_type) < quantity {
            return Err(GameError::ItemUnavailable);
        }
        Ok(())
    }

    pub fn check_deposit(&self, item_type: &ItemType, quantity: u32) -> Result<(), GameError> {
        self.quantity(item_type)
            .checked_add(quantity)
            .ok_or(GameError::Overflow)?;
        Ok(())
    }

    pub fn check_debit(&self, amount: Money) -> Result<(), GameError> {
        self.gold
            .checked_sub(amount)
            .ok_or(GameError::InsufficientFunds)?;
        Ok(())
    }

    pub fn check_credit(&self, amount: Money) -> Result<(), GameError> {
        self.gold.checked_add(amount).ok_or(GameError::Overflow)?;
        Ok(())
    }
}

/// Why the auction house took money out of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakenReason {
    Commission { listing: ListingId },
    ForfeitedDeposit { listing: ListingId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GameError {
    #[error("unknown player {0}")]
    UnknownPlayer(PlayerRef),
    #[error("player {0} already exists")]
    PlayerExists(PlayerRef),
    #[error("unknown item type {0}")]
    UnknownItemType(ItemType),
    #[error("item unavailable")]
    ItemUnavailable,
    #[error("insufficient funds")]
    InsufficientFunds,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cloth() -> ItemType {
        "linen_cloth".into()
    }

    fn alice() -> Player {
        Player::new("alice".into(), Money(100)).with_item(cloth(), 5)
    }

    #[test]
    fn withdraw_needs_the_items() {
        let player = alice();
        assert_eq!(player.check_withdraw(&cloth(), 5), Ok(()));
        assert_eq!(
            player.check_withdraw(&cloth(), 6),
            Err(GameError::ItemUnavailable)
        );
        assert_eq!(
            player.check_withdraw(&"iron_sword".into(), 1),
            Err(GameError::ItemUnavailable)
        );
    }

    #[test]
    fn debit_needs_the_gold() {
        let player = alice();
        assert_eq!(player.check_debit(Money(100)), Ok(()));
        assert_eq!(
            player.check_debit(Money(101)),
            Err(GameError::InsufficientFunds)
        );
    }

    #[test]
    fn credit_and_deposit_detect_overflow() {
        let player = alice();
        assert_eq!(
            player.check_credit(Money(u64::MAX)),
            Err(GameError::Overflow)
        );
        assert_eq!(
            player.check_deposit(&cloth(), u32::MAX),
            Err(GameError::Overflow)
        );
    }
}
