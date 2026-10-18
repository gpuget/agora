//! Outbound ports of the game.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, ItemType, Money};
use game_domain::{CatalogEntry, GameError, Player};

/// Players as projected from the ledger. May lag behind the ledger.
pub trait PlayerView: Send + Sync {
    fn list(&self) -> impl Future<Output = Result<Vec<Player>, GameError>> + Send;
}

pub trait Catalog: Send + Sync {
    fn entry(
        &self,
        item_type: &ItemType,
    ) -> impl Future<Output = Result<Option<CatalogEntry>, GameError>> + Send;

    fn entries(&self) -> impl Future<Output = Result<Vec<CatalogEntry>, GameError>> + Send;
}

/// Auction houses opened by the game. Agora does not list them.
pub trait HouseRegistry: Send + Sync {
    fn add(&self, house: AuctionHouse) -> impl Future<Output = Result<(), GameError>> + Send;

    fn list(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, GameError>> + Send;
}

/// Money taken out of the game by the auction houses, as projected from the ledger. May lag
/// behind the ledger.
pub trait TreasuryView: Send + Sync {
    fn total(&self) -> impl Future<Output = Result<Money, GameError>> + Send;
}
