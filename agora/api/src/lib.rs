//! Inbound ports: the use cases offered by the auction core.
//!
//! The driving side of the Agora hexagon. [`AuctionApi`] is implemented by `AuctionService`
//! in `agora-core`. In this workspace, `game-core` and `tcg-core` call it. All types come
//! from `agora-domain`.
//!
//! Business rules: see `docs/functional.md`.

use std::future::Future;
use std::time::Duration;

use agora_domain::{
    AuctionError, HouseConfig, HouseId, ItemStack, Listing, ListingId, Money, Page, PlayerRef,
    SearchQuery,
};

/// Command of [`AuctionApi::create_listing`]. Prices are per unit.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateListing {
    pub house: HouseId,
    pub seller: PlayerRef,
    /// Stack taken from the seller. At least one item.
    pub item: ItemStack,
    /// Lowest unit price of the first bid. Greater than zero.
    pub start_unit_price: Money,
    /// At least the start price. Required for a commodity.
    pub buyout_unit_price: Option<Money>,
    /// Must be one of the durations allowed by the house.
    pub duration: Duration,
}

/// Result of a successful [`AuctionApi::place_bid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidOutcome {
    /// The bid is now the highest one.
    HighBidder,
    /// The bid reached the buyout price: the listing is sold at the buyout price.
    Bought,
}

/// Use cases of the auction core.
///
/// Money and items move through the outbound ports of `agora-spi`. A listing change is saved
/// before the effects that follow it (deliveries, payments, refunds, sink). Cancel, bid and
/// buyout make up to 3 attempts when another write of the same listing wins the race, then
/// fail with [`AuctionError::Conflict`]. A lost attempt gives back what it debited.
///
/// Every use case can also fail with [`AuctionError::Repository`].
pub trait AuctionApi: Send + Sync {
    /// Validates `config` and stores a new house. Returns its id.
    ///
    /// # Errors
    ///
    /// [`AuctionError::InvalidConfig`] if [`HouseConfig::validate`] fails.
    fn create_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<HouseId, AuctionError>> + Send;

    /// Takes the stack from the seller (escrow) and charges the deposit.
    ///
    /// The deposit is `base_value * quantity * deposit rate`, rounded down, times the deposit
    /// multiplier of the duration. No debit happens when it is zero. The listing copies the
    /// commission and increment settings of the house and ends at now plus `duration`.
    ///
    /// # Errors
    ///
    /// - [`AuctionError::HouseNotFound`], [`AuctionError::InvalidQuantity`],
    ///   [`AuctionError::InvalidPrice`], [`AuctionError::BuyoutBelowStart`],
    ///   [`AuctionError::CommodityNeedsBuyout`], [`AuctionError::DurationNotAllowed`]: the
    ///   command breaks a rule. Nothing moves.
    /// - [`AuctionError::Overflow`]: the highest total price of the stack, the deposit or the
    ///   end time does not fit.
    /// - [`AuctionError::Port`] with [`PortError::ItemUnavailable`](agora_domain::PortError::ItemUnavailable):
    ///   the seller does not hold the stack.
    /// - [`AuctionError::Port`] with [`PortError::InsufficientFunds`](agora_domain::PortError::InsufficientFunds):
    ///   the seller cannot pay the deposit. The stack is given back.
    /// - [`AuctionError::Repository`]: storing the listing failed. The stack and the deposit
    ///   are not given back.
    fn create_listing(
        &self,
        cmd: CreateListing,
    ) -> impl Future<Output = Result<ListingId, AuctionError>> + Send;

    /// Only without bids. Returns the unsold items to the seller, the deposit still held is
    /// forfeited.
    ///
    /// The listing becomes Cancelled. The forfeited deposit goes to the sink. After partial
    /// buyouts of a commodity, only the items still on sale and the deposit still held are
    /// concerned.
    ///
    /// # Errors
    ///
    /// [`AuctionError::ListingNotFound`], [`AuctionError::NotActive`],
    /// [`AuctionError::Expired`] (end time reached), [`AuctionError::NotSeller`],
    /// [`AuctionError::HasBids`], [`AuctionError::Conflict`].
    fn cancel_listing(
        &self,
        seller: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), AuctionError>> + Send;

    /// Bids `unit_price` per unit on the whole stack.
    ///
    /// When the listing has a buyout price and `unit_price` reaches it, this is a buyout of
    /// the whole stack at the buyout price, even for the current high bidder: see
    /// [`AuctionApi::buyout`]. The result is then [`BidOutcome::Bought`].
    ///
    /// Otherwise the total (`unit_price` times the quantity) is debited from the bidder and
    /// held in escrow, the bid becomes the high bid, and the previous high bidder is refunded
    /// their total.
    ///
    /// # Errors
    ///
    /// - [`AuctionError::ListingNotFound`], [`AuctionError::NotActive`],
    ///   [`AuctionError::Expired`] (end time reached).
    /// - [`AuctionError::BidsNotAllowed`]: the listing is a commodity.
    /// - [`AuctionError::SellerCannotBuy`], [`AuctionError::AlreadyHighBidder`].
    /// - [`AuctionError::BidTooLow`]: below [`Listing::min_next_bid`].
    /// - [`AuctionError::Port`] with [`PortError::InsufficientFunds`](agora_domain::PortError::InsufficientFunds):
    ///   the bidder cannot pay the total.
    /// - [`AuctionError::Overflow`], [`AuctionError::Conflict`].
    fn place_bid(
        &self,
        bidder: PlayerRef,
        listing: ListingId,
        unit_price: Money,
    ) -> impl Future<Output = Result<BidOutcome, AuctionError>> + Send;

    /// Buys `quantity` items at the buyout price. A commodity can be bought in part, the
    /// listing then stays open with the rest and the seller gets back the matching share of
    /// the deposit. Any other item must be bought whole.
    ///
    /// Effects: the buyer is debited the buyout unit price times `quantity` and receives the
    /// items. The seller is credited that total, less the commission, plus the deposit share
    /// (see [`Listing::deposit_share`]). The commission goes to the sink. When the whole stack
    /// is bought, the listing becomes Sold and a high bidder, if any, is refunded.
    ///
    /// # Errors
    ///
    /// - [`AuctionError::ListingNotFound`], [`AuctionError::NotActive`],
    ///   [`AuctionError::Expired`] (end time reached).
    /// - [`AuctionError::SellerCannotBuy`], [`AuctionError::NoBuyout`].
    /// - [`AuctionError::QuantityOutOfRange`]: commodity, `quantity` is zero or above what is
    ///   left.
    /// - [`AuctionError::PartialNotAllowed`]: other item, `quantity` is not the whole stack.
    /// - [`AuctionError::Port`] with [`PortError::InsufficientFunds`](agora_domain::PortError::InsufficientFunds):
    ///   the buyer cannot pay the total.
    /// - [`AuctionError::Overflow`], [`AuctionError::Conflict`].
    fn buyout(
        &self,
        buyer: PlayerRef,
        listing: ListingId,
        quantity: u32,
    ) -> impl Future<Output = Result<(), AuctionError>> + Send;

    /// Settles listings past their end time. Must be called periodically by the host.
    /// Returns the number of listings settled.
    ///
    /// Handles at most 100 listings per call, oldest end first. A listing with a high bid
    /// becomes Sold: the high bidder receives the stack, and the seller is credited the bid
    /// total, less the commission, plus the whole deposit still held. A listing without a bid
    /// becomes Expired: the stack goes back to the seller and the deposit is forfeited to the
    /// sink. A listing changed concurrently is skipped and not counted.
    ///
    /// # Errors
    ///
    /// A failure of a port or of the repository stops the batch. Listings settled before the
    /// failure stay settled. A listing whose effect failed stays settled, and the core does not
    /// run its remaining effects again. The rest of the batch is left for a later call.
    fn settle_expired(&self) -> impl Future<Output = Result<usize, AuctionError>> + Send;

    /// Active listings of one house matching `query`, sorted by unit price then end time.
    ///
    /// # Errors
    ///
    /// [`AuctionError::InvalidPage`] if `query.limit` is 0 or above 100.
    fn search(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, AuctionError>> + Send;
}
