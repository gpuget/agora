//! Outbound ports: what the auction core needs from the outside.
//!
//! Every call that moves money or items carries an [`OpId`]. Implementations should ignore
//! an `OpId` they already applied, so that a replayed call has no effect.

use std::future::Future;

use agora_domain::{
    AuctionHouse, HouseId, ItemStack, Listing, ListingId, Money, OpId, Page, PlayerRef, PortError,
    SearchQuery, Timestamp,
};
use thiserror::Error;

/// Player inventories, owned by the game.
pub trait Inventory: Send + Sync {
    /// Removes the stack from the player inventory. Fails with
    /// [`PortError::ItemUnavailable`] if the player does not hold it.
    fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> impl Future<Output = Result<(), PortError>> + Send;

    fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

/// Player money, owned by the game.
pub trait Wallet: Send + Sync {
    /// Fails with [`PortError::InsufficientFunds`] if the balance is too low.
    fn debit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), PortError>> + Send;

    fn credit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkReason {
    Commission { listing: ListingId },
    ForfeitedDeposit { listing: ListingId },
}

/// Receives the money taken by the auction house: commissions and forfeited deposits.
/// The game decides what happens to it (destroyed, credited to a treasury, ...).
pub trait MoneySink: Send + Sync {
    fn absorb(
        &self,
        op: OpId,
        house: HouseId,
        amount: Money,
        reason: SinkReason,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepoError {
    /// The stored version differs from the expected one.
    #[error("version conflict")]
    Conflict,
    #[error("{0}")]
    Other(String),
}

pub trait AuctionRepository: Send + Sync {
    fn insert_house(
        &self,
        house: &AuctionHouse,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    fn get_house(
        &self,
        id: HouseId,
    ) -> impl Future<Output = Result<Option<AuctionHouse>, RepoError>> + Send;

    fn insert_listing(
        &self,
        listing: &Listing,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    fn get_listing(
        &self,
        id: ListingId,
    ) -> impl Future<Output = Result<Option<Listing>, RepoError>> + Send;

    /// Replaces the stored listing only if its version equals `expected_version`,
    /// otherwise fails with [`RepoError::Conflict`].
    fn update_listing(
        &self,
        listing: &Listing,
        expected_version: u64,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    /// Active listings with `ends_at <= now`, oldest end first.
    fn find_expired(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Listing>, RepoError>> + Send;

    /// Active listings matching the query, sorted by [`Listing::sort_unit_price`] then `ends_at`.
    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, RepoError>> + Send;
}

/// Passive clock: the core reads it, it never wakes the core up.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
