//! Event-sourced ledger of a game on Agora: the money and items of each player, and the money
//! taken out of the game by the auction houses.
//!
//! Reuses the Agora vocabulary (`Money`, `PlayerRef`, `ItemType`) since the game trades on it.
//! Every movement carries the [`OpId`](agora_domain::OpId) of the Agora call, so a replayed
//! call records nothing. [`AgoraBridge`] lets Agora drive a [`LedgerApi`] through its
//! outbound ports.
//!
//! Both demo games use this crate. An [`Account`] holds the money and items of a player in
//! the game (gold in copper, items) and of a collector in the tcg (dust, cards).
//!
//! # Streams
//!
//! - One account stream per player, keyed by [`PlayerRef`], run by [`AccountDecider`].
//! - One treasury stream, keyed by [`TreasuryId`], run by [`TreasuryDecider`]. It records the
//!   commissions and forfeited deposits. Agora's `MoneySink` port is backed by it.
//!
//! [`LedgerService`] runs the commands on an [`EventStore`](event_sourcing::EventStore) per
//! stream type. Read models project the global log of these stores. [`Account::apply`] folds
//! account events for both the aggregate and the read models.
//!
//! # Idempotence
//!
//! Each stream remembers the [`OpId`](agora_domain::OpId)s it recorded. A command with an
//! already applied `OpId` records nothing and succeeds, even if the same movement would now
//! fail. The check is per stream: the same `OpId` on two accounts records on both.

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
    /// Commission on a sale of `listing`, taken from the seller's proceeds.
    Commission {
        /// The listing sold.
        listing: ListingId,
    },
    /// Part of the deposit of `listing` not returned to the seller: the share of the items
    /// that stayed unsold, on cancellation or expiry.
    ForfeitedDeposit {
        /// The listing cancelled or expired.
        listing: ListingId,
    },
}

/// Failure of a ledger call. A failed call records no event.
///
/// [`AgoraBridge`] maps [`InsufficientFunds`](Self::InsufficientFunds) and
/// [`ItemUnavailable`](Self::ItemUnavailable) to the matching
/// [`PortError`](agora_domain::PortError) and every other variant to `PortError::Other`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerError {
    /// No account was opened for this player. Returned by every movement on it.
    #[error("unknown account {0}")]
    UnknownAccount(PlayerRef),
    /// [`LedgerApi::open_account`] on a player that already has an account.
    #[error("account {0} already exists")]
    AccountExists(PlayerRef),
    /// Withdrawal of more items than the account holds.
    #[error("item unavailable")]
    ItemUnavailable,
    /// Debit of more money than the account holds.
    #[error("insufficient funds")]
    InsufficientFunds,
    /// A credit, a deposit or the treasury total would exceed the maximum of its type
    /// (`u64` for money, `u32` for a quantity).
    #[error("amount overflow")]
    Overflow,
    /// The event store failed. [`StoreError::Conflict`] means the stream kept changing
    /// during every retry of [`event_sourcing::execute`].
    #[error(transparent)]
    Store(#[from] StoreError),
}
