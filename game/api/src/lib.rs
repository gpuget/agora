//! Inbound ports of the game.

use std::future::Future;
use std::time::Duration;

pub use agora_api::BidOutcome;
use agora_domain::{
    AuctionHouse, HouseConfig, HouseId, ItemType, Listing, ListingId, Money, OpId, Page, PlayerRef,
    SearchQuery,
};
use game_domain::{CatalogEntry, GameError, Player, TakenReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellCommand {
    pub player: PlayerRef,
    pub house: HouseId,
    pub item_type: ItemType,
    pub quantity: u32,
    pub start_unit_price: Money,
    pub buyout_unit_price: Option<Money>,
    pub duration: Duration,
}

/// What players (through the ui) and the game host can do.
pub trait GameApi: Send + Sync {
    fn open_auction_house(
        &self,
        config: HouseConfig,
    ) -> impl Future<Output = Result<AuctionHouse, GameError>> + Send;

    fn auction_houses(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, GameError>> + Send;

    /// Read from a projection of the ledger: may lag behind the last ledger calls.
    fn players(&self) -> impl Future<Output = Result<Vec<Player>, GameError>> + Send;

    fn catalog(&self) -> impl Future<Output = Result<Vec<CatalogEntry>, GameError>> + Send;

    /// Total money taken out of the game by the auction houses. Read from a projection of
    /// the ledger: may lag behind the last ledger calls.
    fn money_taken(&self) -> impl Future<Output = Result<Money, GameError>> + Send;

    fn search_market(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Page<Listing>, GameError>> + Send;

    /// Puts a stack up for sale. The base value comes from the catalog.
    fn sell(&self, cmd: SellCommand) -> impl Future<Output = Result<ListingId, GameError>> + Send;

    fn bid(
        &self,
        player: PlayerRef,
        listing: ListingId,
        unit_price: Money,
    ) -> impl Future<Output = Result<BidOutcome, GameError>> + Send;

    fn buyout(
        &self,
        player: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    fn cancel(
        &self,
        player: PlayerRef,
        listing: ListingId,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    /// Settles expired auctions. Must be called periodically by the host.
    fn settle_auctions(&self) -> impl Future<Output = Result<usize, GameError>> + Send;
}

/// Gold and items of the players, driven by the auction house through an adapter.
///
/// Every movement carries the [`OpId`] of the Agora call: a call replayed with the same
/// `OpId` succeeds without effect.
pub trait LedgerApi: Send + Sync {
    /// Opens the account of a new player with their starting gold and items. Fails with
    /// [`GameError::PlayerExists`] if the player already has one.
    fn open_player(&self, player: Player) -> impl Future<Output = Result<(), GameError>> + Send;

    fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    fn debit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    fn credit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), GameError>> + Send;

    /// Money taken by the auction house (commission, forfeited deposit).
    fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> impl Future<Output = Result<(), GameError>> + Send;
}
