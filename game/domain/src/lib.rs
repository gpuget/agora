//! Domain of the demo game: players, their gold and items, and the item catalog.
//!
//! Reuses the Agora vocabulary (`Money`, `PlayerRef`, `ItemType`) since the game trades on it.

use std::collections::BTreeMap;

use agora_domain::{AuctionError, ItemType, ListingId, Money, PlayerRef};
use thiserror::Error;

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

    pub fn withdraw(&mut self, item_type: &ItemType, quantity: u32) -> Result<(), GameError> {
        let held = self
            .items
            .get_mut(item_type)
            .filter(|held| **held >= quantity)
            .ok_or(GameError::ItemUnavailable)?;
        *held -= quantity;
        if *held == 0 {
            self.items.remove(item_type);
        }
        Ok(())
    }

    pub fn deposit(&mut self, item_type: &ItemType, quantity: u32) -> Result<(), GameError> {
        let held = self.items.entry(item_type.clone()).or_default();
        *held = held.checked_add(quantity).ok_or(GameError::Overflow)?;
        Ok(())
    }

    pub fn debit(&mut self, amount: Money) -> Result<(), GameError> {
        self.gold = self
            .gold
            .checked_sub(amount)
            .ok_or(GameError::InsufficientFunds)?;
        Ok(())
    }

    pub fn credit(&mut self, amount: Money) -> Result<(), GameError> {
        self.gold = self.gold.checked_add(amount).ok_or(GameError::Overflow)?;
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
    fn withdraw_removes_empty_stacks() {
        let mut player = alice();
        player.withdraw(&cloth(), 5).unwrap();
        assert!(player.items.is_empty());
    }

    #[test]
    fn withdraw_more_than_held_fails_without_change() {
        let mut player = alice();
        assert_eq!(
            player.withdraw(&cloth(), 6),
            Err(GameError::ItemUnavailable)
        );
        assert_eq!(player.quantity(&cloth()), 5);
    }

    #[test]
    fn debit_more_than_gold_fails_without_change() {
        let mut player = alice();
        assert_eq!(player.debit(Money(101)), Err(GameError::InsufficientFunds));
        assert_eq!(player.gold, Money(100));
    }

    #[test]
    fn credit_and_deposit_detect_overflow() {
        let mut player = alice();
        assert_eq!(player.credit(Money(u64::MAX)), Err(GameError::Overflow));
        assert_eq!(player.deposit(&cloth(), u32::MAX), Err(GameError::Overflow));
    }
}
