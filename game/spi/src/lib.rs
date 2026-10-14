//! Outbound ports of the game.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, ItemType, Money, PlayerRef};
use game_domain::{CatalogEntry, GameError, Player};

pub trait PlayerRepository: Send + Sync {
    fn insert(&self, player: Player) -> impl Future<Output = Result<(), GameError>> + Send;

    fn list(&self) -> impl Future<Output = Result<Vec<Player>, GameError>> + Send;

    /// Applies `change` to the player atomically: the change is stored only if it
    /// returns `Ok`. Fails with [`GameError::UnknownPlayer`] if the player does not exist.
    fn update<T, F>(
        &self,
        id: &PlayerRef,
        change: F,
    ) -> impl Future<Output = Result<T, GameError>> + Send
    where
        T: Send,
        F: FnOnce(&mut Player) -> Result<T, GameError> + Send;
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

/// Money taken out of the game by the auction houses.
pub trait Treasury: Send + Sync {
    fn add(&self, amount: Money) -> impl Future<Output = Result<(), GameError>> + Send;

    fn total(&self) -> impl Future<Output = Result<Money, GameError>> + Send;
}
