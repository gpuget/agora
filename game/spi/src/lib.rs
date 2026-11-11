//! Outbound ports of the game.
//!
//! [`PlayerView`] and [`TreasuryView`] read projections of the ledger, which may lag behind
//! it. `game_core::CaughtUpView` wraps a projection so that a read sees every event recorded
//! before the call.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, ItemType, Money};
use game_domain::{CatalogEntry, GameError, Player};

/// Players as projected from the ledger. May lag behind the ledger.
pub trait PlayerView: Send + Sync {
    /// Every player with an opened account.
    fn list(&self) -> impl Future<Output = Result<Vec<Player>, GameError>> + Send;
}

/// The item types the game sells, with their base value and commodity flag.
pub trait Catalog: Send + Sync {
    /// `None` if `item_type` is not in the catalog.
    fn entry(
        &self,
        item_type: &ItemType,
    ) -> impl Future<Output = Result<Option<CatalogEntry>, GameError>> + Send;

    /// Every entry of the catalog.
    fn entries(&self) -> impl Future<Output = Result<Vec<CatalogEntry>, GameError>> + Send;
}

/// Auction houses opened by the game. Agora does not list them.
pub trait HouseRegistry: Send + Sync {
    /// Records a house created in Agora.
    fn add(&self, house: AuctionHouse) -> impl Future<Output = Result<(), GameError>> + Send;

    /// Every recorded house.
    fn list(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, GameError>> + Send;
}

/// Money taken out of the game by the auction houses, as projected from the ledger. May lag
/// behind the ledger.
pub trait TreasuryView: Send + Sync {
    /// Sum of the commissions and forfeited deposits, in copper.
    fn total(&self) -> impl Future<Output = Result<Money, GameError>> + Send;
}
