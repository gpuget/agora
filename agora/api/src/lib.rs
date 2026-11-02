//! Inbound ports: the use cases offered by the auction core.

use std::future::Future;
use std::time::Duration;

use agora_domain::{
    AuctionError, HouseConfig, HouseId, ItemStack, Listing, ListingId, Money, Page, PlayerRef,
    SearchQuery,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateListing {
    pub house: HouseId,
    pub seller: PlayerRef,
    pub item: ItemStack,
    pub start_unit_price: Money,
    pub buyout_unit_price: Option<Money>,
    /// Must be one of the durations allowed by the house.
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidOutcome {
    /// The bid is now the highest one.
    HighBidder,
    /// The bid reached the buyout price: the listing is sold at the buyout price.
    Bought,
}

pub trait AuctionApi: Send + Sync {
    fn create_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<HouseId, AuctionError>> + Send;

    /// Takes the stack from the seller (escrow) and charges the deposit.
    fn create_listing(
        &self,
        cmd: CreateListing,
    ) -> impl Future<Output = Result<ListingId, AuctionError>> + Send;

    /// Only without bids. Returns the unsold items to the seller, the deposit still held is
    /// forfeited.
    fn cancel_listing(
        &self,
        seller: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), AuctionError>> + Send;

    fn place_bid(
        &self,
        bidder: PlayerRef,
        listing: ListingId,
        unit_price: Money,
    ) -> impl Future<Output = Result<BidOutcome, AuctionError>> + Send;

    /// Buys `quantity` items at the buyout price. A commodity can be bought in part, the
    /// listing then stays open with the rest and the seller gets back the matching share of
    /// the deposit. Any other item must be bought whole.
    fn buyout(
        &self,
        buyer: PlayerRef,
        listing: ListingId,
        quantity: u32,
    ) -> impl Future<Output = Result<(), AuctionError>> + Send;

    /// Settles listings past their end time. Must be called periodically by the host.
    /// Returns the number of listings settled.
    fn settle_expired(&self) -> impl Future<Output = Result<usize, AuctionError>> + Send;

    fn search(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, AuctionError>> + Send;
}
