//! Outbound ports: what the auction core needs from the outside.
//!
//! Every call that moves money or items carries an [`OpId`]. Implementations should ignore
//! an `OpId` they already applied, so that a replayed call has no effect.
//!
//! The driven side of the Agora hexagon, called by `AuctionService` in `agora-core`. In this
//! workspace, `AgoraBridge` in the `ledger` crate implements [`Inventory`], [`Wallet`] and
//! [`MoneySink`] on top of the game ledger. The game and tcg servers provide the [`Clock`].
//! The `memory` feature of `agora-core` provides in-memory adapters for every port, including
//! the [`AuctionRepository`] used by those servers.
//!
//! Amounts are [`Money`] in the smallest unit of the game's currency.

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
    ///
    /// Called when a listing is created, to hold the stack in escrow.
    fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> impl Future<Output = Result<(), PortError>> + Send;

    /// Adds the stack to the player inventory.
    ///
    /// Called to deliver sold items to the buyer, to return unsold items to the seller, and
    /// to give back a stack whose deposit could not be paid.
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
    ///
    /// Takes the deposit from the seller and the total of a bid or a buyout from the player.
    fn debit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), PortError>> + Send;

    /// Pays the seller, refunds a high bidder who is outbid or bought out, or gives back a
    /// debit whose attempt was lost.
    fn credit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

/// Why the core sends money to the [`MoneySink`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkReason {
    /// Commission on a sale, in full or in part, of `listing`.
    Commission { listing: ListingId },
    /// Deposit still held when `listing` was cancelled or expired.
    ForfeitedDeposit { listing: ListingId },
}

/// Receives the money taken by the auction house: commissions and forfeited deposits.
/// The game decides what happens to it (destroyed, credited to a treasury, ...).
pub trait MoneySink: Send + Sync {
    /// Takes `amount`, always greater than zero, on behalf of `house`.
    fn absorb(
        &self,
        op: OpId,
        house: HouseId,
        amount: Money,
        reason: SinkReason,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

/// Failure of an [`AuctionRepository`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepoError {
    /// The stored version differs from the expected one.
    ///
    /// On [`AuctionRepository::update_listing`], the core retries or skips the listing.
    /// Anywhere else it becomes [`AuctionError::Conflict`](agora_domain::AuctionError::Conflict).
    #[error("version conflict")]
    Conflict,
    /// Any other storage failure. Becomes
    /// [`AuctionError::Repository`](agora_domain::AuctionError::Repository).
    #[error("{0}")]
    Other(String),
}

/// Storage of houses and listings.
pub trait AuctionRepository: Send + Sync {
    /// Stores a new house.
    fn insert_house(
        &self,
        house: &AuctionHouse,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    /// `None` if no house has this id.
    fn get_house(
        &self,
        id: HouseId,
    ) -> impl Future<Output = Result<Option<AuctionHouse>, RepoError>> + Send;

    /// Stores a new listing, at version 0.
    fn insert_listing(
        &self,
        listing: &Listing,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    /// The listing whatever its status, `None` if no listing has this id.
    fn get_listing(
        &self,
        id: ListingId,
    ) -> impl Future<Output = Result<Option<Listing>, RepoError>> + Send;

    /// Replaces the stored listing only if its version equals `expected_version`,
    /// otherwise fails with [`RepoError::Conflict`].
    ///
    /// `listing.version` is already incremented by the core.
    fn update_listing(
        &self,
        listing: &Listing,
        expected_version: u64,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;

    /// Active listings with `ends_at <= now`, oldest end first.
    ///
    /// At most `limit` listings.
    fn find_expired(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Listing>, RepoError>> + Send;

    /// Active listings matching the query, sorted by [`Listing::sort_unit_price`] then `ends_at`.
    ///
    /// Applies `offset` and `limit`. [`Page::total`] counts every match. The core checks
    /// `limit` before the call.
    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, RepoError>> + Send;
}

/// Passive clock: the core reads it, it never wakes the core up.
pub trait Clock: Send + Sync {
    /// Current time.
    fn now(&self) -> Timestamp;
}
