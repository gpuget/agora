//! Outbound ports of the trading card game.
//!
//! [`CollectorView`] and [`TreasuryView`] read projections of the ledger, which may lag
//! behind it. `tcg_core::CaughtUpView` wraps a projection so that a read sees every event
//! recorded before the call.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, Money};
use tcg_domain::{CardEntry, CardId, Collector, TcgError};

/// Collectors as projected from the ledger. May lag behind the ledger.
pub trait CollectorView: Send + Sync {
    /// Every collector with an opened account.
    fn list(&self) -> impl Future<Output = Result<Vec<Collector>, TcgError>> + Send;
}

/// The cards the game sells, with their rarity and base value.
pub trait Catalog: Send + Sync {
    /// `None` if `card` is not in the catalog.
    fn entry(
        &self,
        card: &CardId,
    ) -> impl Future<Output = Result<Option<CardEntry>, TcgError>> + Send;

    /// Every entry of the catalog.
    fn entries(&self) -> impl Future<Output = Result<Vec<CardEntry>, TcgError>> + Send;
}

/// Auction houses opened by the game. Agora does not list them.
pub trait HouseRegistry: Send + Sync {
    /// Records a house created in Agora.
    fn add(&self, house: AuctionHouse) -> impl Future<Output = Result<(), TcgError>> + Send;

    /// Every recorded house.
    fn list(&self) -> impl Future<Output = Result<Vec<AuctionHouse>, TcgError>> + Send;
}

/// Dust taken out of the game by the auction houses, as projected from the ledger. May lag
/// behind the ledger.
pub trait TreasuryView: Send + Sync {
    /// Sum of the commissions and forfeited deposits, in dust.
    fn total(&self) -> impl Future<Output = Result<Money, TcgError>> + Send;
}
