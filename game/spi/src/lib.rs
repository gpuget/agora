//! Outbound ports of the game.

use std::future::Future;

/// The auction house is an external system for the game: its inbound port is an
/// outbound port of the game.
pub use agora_api::AuctionApi;
use agora_domain::{AuctionHouse, ItemType, Money};
use game_domain::{CatalogEntry, GameError, Player};

/// One event of the global log of an event store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded<I, E> {
    /// Position in the global log, starting at 1.
    pub position: u64,
    pub stream: I,
    pub event: E,
}

/// Append-only log of events, split into streams identified by `I`.
pub trait EventStore<I, E>: Send + Sync {
    /// Events of one stream, in order. Empty if the stream does not exist.
    fn load(&self, stream: &I) -> impl Future<Output = Result<Vec<E>, GameError>> + Send;

    /// Appends `events` to a stream if it still holds `expected` events. Fails with
    /// [`GameError::Conflict`] otherwise. Returns the new number of events of the stream.
    fn append(
        &self,
        stream: &I,
        expected: u64,
        events: Vec<E>,
    ) -> impl Future<Output = Result<u64, GameError>> + Send;

    /// At most `limit` events of all streams with a position greater than `after`, in
    /// position order.
    fn read_all(
        &self,
        after: u64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Recorded<I, E>>, GameError>> + Send;
}

/// Read model fed by a projector from the global log of an [`EventStore`].
pub trait Projection<I, E>: Send + Sync {
    /// Position of the last applied event, 0 if none.
    fn checkpoint(&self) -> impl Future<Output = Result<u64, GameError>> + Send;

    /// Applies `events` in order and moves the checkpoint to the last one, atomically.
    /// Skips events at or below the checkpoint, so a replayed batch has no effect.
    fn apply(
        &self,
        events: &[Recorded<I, E>],
    ) -> impl Future<Output = Result<(), GameError>> + Send;
}

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
