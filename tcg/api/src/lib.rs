//! Inbound ports of the trading card game.

use std::future::Future;
use std::time::Duration;

pub use agora_api::BidOutcome;
use agora_domain::{
    AuctionHouse, HouseConfig, HouseId, Listing, ListingId, Money, OpId, Page, PlayerRef,
    SearchQuery,
};
use tcg_domain::{CardEntry, CardId, Collector, TakenReason, TcgError};

/// Puts one copy of a card up for sale: a listing always holds a single card, so its prices
/// are the prices of that card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellCommand {
    pub collector: PlayerRef,
    pub house: HouseId,
    pub card: CardId,
    pub start_price: Money,
    pub buyout_price: Option<Money>,
    pub duration: Duration,
}

/// What collectors (through the ui) and the game host can do.
pub trait TcgApi: Send + Sync {
    fn open_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<AuctionHouse, TcgError>> + Send;

    fn auction_houses(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, TcgError>> + Send;

    /// Read from a projection of the ledger: may lag behind the last ledger calls.
    fn collectors(&self) -> impl Future<Output = Result<Vec<Collector>, TcgError>> + Send;

    fn catalog(&self) -> impl Future<Output = Result<Vec<CardEntry>, TcgError>> + Send;

    /// Total dust taken out of the game by the auction houses. Read from a projection of
    /// the ledger: may lag behind the last ledger calls.
    fn dust_taken(&self) -> impl Future<Output = Result<Money, TcgError>> + Send;

    fn search_market(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, TcgError>> + Send;

    /// Puts one copy of a card up for sale. The base value comes from the catalog.
    fn sell(&self, cmd: SellCommand) -> impl Future<Output = Result<ListingId, TcgError>> + Send;

    fn bid(
        &self,
        collector: PlayerRef,
        listing: ListingId,
        price: Money,
    ) -> impl Future<Output = Result<BidOutcome, TcgError>> + Send;

    fn buyout(
        &self,
        collector: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn cancel(
        &self,
        collector: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    /// Settles expired auctions. Must be called periodically by the host.
    fn settle_auctions(&self) -> impl Future<Output = Result<usize, TcgError>> + Send;
}

/// Dust and cards of the collectors, driven by the auction house through an adapter.
///
/// Every movement carries the [`OpId`] of the Agora call: a call replayed with the same
/// `OpId` succeeds without effect.
pub trait LedgerApi: Send + Sync {
    /// Opens the account of a new collector with their starting dust and cards. Fails with
    /// [`TcgError::CollectorExists`] if the collector already has one.
    fn open_collector(
        &self,
        collector: Collector,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn withdraw_card(
        &self,
        op: OpId,
        collector: &PlayerRef,
        card: &CardId,
        copies: u32,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn deposit_card(
        &self,
        op: OpId,
        collector: &PlayerRef,
        card: &CardId,
        copies: u32,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn debit(
        &self,
        op: OpId,
        collector: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn credit(
        &self,
        op: OpId,
        collector: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    /// Dust taken by the auction house (commission, forfeited deposit).
    fn take_dust(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;
}
