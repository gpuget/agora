//! Event-sourced ledger of a game on Agora: the money and items of each player, and the money
//! taken out of the game by the auction houses.
//!
//! Reuses the Agora vocabulary (`Money`, `PlayerRef`, `ItemType`) since the game trades on it.
//! Every movement carries the [`OpId`](agora_domain::OpId) of the Agora call, so a replayed
//! call records nothing. [`AgoraBridge`] lets Agora drive a [`LedgerApi`] through its
//! outbound ports.

use agora_domain::{ListingId, PlayerRef};
use event_sourcing::StoreError;
use thiserror::Error;

mod account;
mod api;
mod bridge;
mod service;
mod treasury;

pub use account::{Account, AccountCommand, AccountDecider, AccountEvent, AccountState};
pub use api::LedgerApi;
pub use bridge::AgoraBridge;
pub use service::LedgerService;
pub use treasury::{TreasuryCommand, TreasuryDecider, TreasuryEvent, TreasuryId, TreasuryState};

/// Why the auction house took money out of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakenReason {
    Commission { listing: ListingId },
    ForfeitedDeposit { listing: ListingId },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerError {
    #[error("unknown account {0}")]
    UnknownAccount(PlayerRef),
    #[error("account {0} already exists")]
    AccountExists(PlayerRef),
    #[error("item unavailable")]
    ItemUnavailable,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("amount overflow")]
    Overflow,
    #[error(transparent)]
    Store(#[from] StoreError),
}
