//! Outbound ports of the trading card game.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, Money};
use tcg_domain::{CardEntry, CardId, Collector, TcgError};

/// Collectors as projected from the ledger. May lag behind the ledger.
pub trait CollectorView: Send + Sync {
    fn list(&self) -> impl Future<Output = Result<Vec<Collector>, TcgError>> + Send;
}

pub trait Catalog: Send + Sync {
    fn entry(
        &self,
        card: &CardId,
    ) -> impl Future<Output = Result<Option<CardEntry>, TcgError>> + Send;

    fn entries(&self) -> impl Future<Output = Result<Vec<CardEntry>, TcgError>> + Send;
}

/// Auction houses opened by the game. Agora does not list them.
pub trait HouseRegistry: Send + Sync {
    fn add(&self, house: AuctionHouse) -> impl Future<Output = Result<(), TcgError>> + Send;

    fn list(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, TcgError>> + Send;
}

/// Dust taken out of the game by the auction houses, as projected from the ledger. May lag
/// behind the ledger.
pub trait TreasuryView: Send + Sync {
    fn total(&self) -> impl Future<Output = Result<Money, TcgError>> + Send;
}
