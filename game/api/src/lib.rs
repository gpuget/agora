//! Inbound ports of the game.
//!
//! Amounts are in copper. Prices are per unit: the total of a stack is unit price * quantity.
//! Agora rejections arrive as [`GameError::Auction`].

use std::future::Future;
use std::time::Duration;

pub use agora_api::BidOutcome;
use agora_domain::{
    AuctionHouse, HouseConfig, HouseId, ItemType, Listing, ListingId, Money, Page, PlayerRef,
    SearchQuery,
};
use game_domain::{CatalogEntry, GameError, Player};

/// Input of [`GameApi::sell`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellCommand {
    /// The seller.
    pub player: PlayerRef,
    pub house: HouseId,
    /// Must be in the catalog.
    pub item_type: ItemType,
    /// Number of items in the stack.
    pub quantity: u32,
    /// Per unit, in copper.
    pub start_unit_price: Money,
    /// Per unit, in copper. Required for a commodity.
    pub buyout_unit_price: Option<Money>,
    /// Must be one of the durations allowed by the house.
    pub duration: Duration,
}

/// What players (through the ui) and the game host can do.
pub trait GameApi: Send + Sync {
    /// Creates an auction house in Agora and registers it, so that
    /// [`auction_houses`](Self::auction_houses) lists it.
    fn open_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<AuctionHouse, GameError>> + Send;

    /// The houses opened with [`open_auction_house`](Self::open_auction_house).
    fn auction_houses(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, GameError>> + Send;

    /// Read from a projection of the ledger: may lag behind the last ledger calls.
    fn players(&self) -> impl Future<Output = Result<Vec<Player>, GameError>> + Send;

    /// Every item type the game can sell.
    fn catalog(&self) -> impl Future<Output = Result<Vec<CatalogEntry>, GameError>> + Send;

    /// Total money taken out of the game by the auction houses. Read from a projection of
    /// the ledger: may lag behind the last ledger calls.
    ///
    /// In copper: the sum of commissions and forfeited deposits of every house.
    fn money_taken(&self) -> impl Future<Output = Result<Money, GameError>> + Send;

    /// Active listings of one house matching `query`.
    fn search_market(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, GameError>> + Send;

    /// Puts a stack up for sale. The base value comes from the catalog.
    ///
    /// Fails with [`GameError::UnknownItemType`] if the item type is not in the catalog.
    /// Agora takes the stack from the player into escrow and debits the deposit.
    fn sell(&self, cmd: SellCommand) -> impl Future<Output = Result<ListingId, GameError>> + Send;

    /// Bids `unit_price` per unit on an auction. Agora holds unit price * stack quantity of
    /// the player in escrow until the bid is outbid or settled. A bid at or above the buyout
    /// price buys the whole stack at the buyout price instead: see [`BidOutcome`].
    fn bid(
        &self,
        player: PlayerRef,
        listing: ListingId,
        unit_price: Money,
    ) -> impl Future<Output = Result<BidOutcome, GameError>> + Send;

    /// Buys `quantity` items. Only a commodity can be bought in part.
    ///
    /// Pays buyout price * `quantity`. The listing must have a buyout price.
    fn buyout(
        &self,
        player: PlayerRef,
        listing: ListingId,
        quantity: u32,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    /// Cancels a listing of `player` that has no bid. The unsold items go back to the
    /// seller; the deposit still held is forfeited.
    fn cancel(
        &self,
        player: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    /// Settles expired auctions. Must be called periodically by the host.
    ///
    /// Returns the number of listings settled by this call.
    fn settle_auctions(&self) -> impl Future<Output = Result<usize, GameError>> + Send;
}
