//! Inbound ports of the trading card game.
//!
//! Amounts are in dust. A listing always holds one copy of a card, so a price is both the
//! unit price and the total. Agora rejections arrive as [`TcgError::Auction`].

use std::future::Future;
use std::time::Duration;

pub use agora_api::BidOutcome;
use agora_domain::{
    AuctionHouse, HouseConfig, HouseId, Listing, ListingId, Money, Page, PlayerRef, SearchQuery,
};
use tcg_domain::{CardEntry, CardId, Collector, TcgError};

/// Puts one copy of a card up for sale: a listing always holds a single card, so its prices
/// are the prices of that card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellCommand {
    /// The seller.
    pub collector: PlayerRef,
    pub house: HouseId,
    /// Must be in the catalog.
    pub card: CardId,
    /// In dust.
    pub start_price: Money,
    /// In dust.
    pub buyout_price: Option<Money>,
    /// Must be one of the durations allowed by the house.
    pub duration: Duration,
}

/// What collectors (through the ui) and the game host can do.
pub trait TcgApi: Send + Sync {
    /// Creates an auction house in Agora and registers it, so that
    /// [`auction_houses`](Self::auction_houses) lists it.
    fn open_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<AuctionHouse, TcgError>> + Send;

    /// The houses opened with [`open_auction_house`](Self::open_auction_house).
    fn auction_houses(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, TcgError>> + Send;

    /// Read from a projection of the ledger: may lag behind the last ledger calls.
    fn collectors(&self) -> impl Future<Output = Result<Vec<Collector>, TcgError>> + Send;

    /// Every card the game can sell.
    fn catalog(&self) -> impl Future<Output = Result<Vec<CardEntry>, TcgError>> + Send;

    /// Total dust taken out of the game by the auction houses. Read from a projection of
    /// the ledger: may lag behind the last ledger calls.
    ///
    /// The sum of commissions and forfeited deposits of every house.
    fn dust_taken(&self) -> impl Future<Output = Result<Money, TcgError>> + Send;

    /// Active listings of one house matching `query`.
    fn search_market(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, TcgError>> + Send;

    /// Puts one copy of a card up for sale. The base value comes from the catalog.
    ///
    /// Fails with [`TcgError::UnknownCard`] if the card is not in the catalog. Agora takes
    /// the copy from the collector into escrow and debits the deposit.
    fn sell(&self, cmd: SellCommand) -> impl Future<Output = Result<ListingId, TcgError>> + Send;

    /// Bids `price` on an auction. Agora holds `price` of the collector in escrow until the
    /// bid is outbid or settled. A bid at or above the buyout price buys the card at the
    /// buyout price instead: see [`BidOutcome`].
    fn bid(
        &self,
        collector: PlayerRef,
        listing: ListingId,
        price: Money,
    ) -> impl Future<Output = Result<BidOutcome, TcgError>> + Send;

    /// Buys the card at its buyout price. The listing must have a buyout price.
    fn buyout(
        &self,
        collector: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    /// Cancels a listing of `collector` that has no bid. The card goes back to the seller;
    /// the deposit is forfeited.
    fn cancel(
        &self,
        collector: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), TcgError>> + Send;

    /// Settles expired auctions. Must be called periodically by the host.
    ///
    /// Returns the number of listings settled by this call.
    fn settle_auctions(&self) -> impl Future<Output = Result<usize, TcgError>> + Send;
}
