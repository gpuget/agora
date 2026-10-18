//! In-memory adapters of the game outbound ports. Cheap to clone: clones share state.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agora_domain::{AuctionHouse, ItemType, Money, PlayerRef};
use game_domain::{CatalogEntry, GameError, Player, PlayerEvent, TreasuryEvent, TreasuryId};
use game_spi::{
    Catalog, EventStore, HouseRegistry, PlayerView, Projection, Recorded, TreasuryView,
};

struct Log<I, E> {
    streams: BTreeMap<I, Vec<E>>,
    /// The global log: the event at index `i` has position `i + 1`.
    all: Vec<Recorded<I, E>>,
}

pub struct InMemoryEventStore<I, E>(Arc<Mutex<Log<I, E>>>);

impl<I, E> Default for InMemoryEventStore<I, E> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Log {
            streams: BTreeMap::new(),
            all: Vec::new(),
        })))
    }
}

impl<I, E> Clone for InMemoryEventStore<I, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<I, E> EventStore<I, E> for InMemoryEventStore<I, E>
where
    I: Ord + Clone + Send + Sync,
    E: Clone + Send + Sync,
{
    async fn load(&self, stream: &I) -> Result<Vec<E>, GameError> {
        let log = self.0.lock().unwrap();
        Ok(log.streams.get(stream).cloned().unwrap_or_default())
    }

    async fn append(&self, stream: &I, expected: u64, events: Vec<E>) -> Result<u64, GameError> {
        let mut log = self.0.lock().unwrap();
        let Log { streams, all } = &mut *log;
        let recorded = streams.entry(stream.clone()).or_default();
        if recorded.len() as u64 != expected {
            return Err(GameError::Conflict);
        }
        for event in events {
            all.push(Recorded {
                position: all.len() as u64 + 1,
                stream: stream.clone(),
                event: event.clone(),
            });
            recorded.push(event);
        }
        Ok(recorded.len() as u64)
    }

    async fn read_all(&self, after: u64, limit: usize) -> Result<Vec<Recorded<I, E>>, GameError> {
        let log = self.0.lock().unwrap();
        let start = usize::try_from(after).unwrap_or(usize::MAX);
        Ok(log.all.iter().skip(start).take(limit).cloned().collect())
    }
}

#[derive(Default)]
struct PlayersState {
    checkpoint: u64,
    players: BTreeMap<PlayerRef, Player>,
}

/// Players folded from the player events of the ledger.
#[derive(Clone, Default)]
pub struct InMemoryPlayerProjection(Arc<Mutex<PlayersState>>);

impl InMemoryPlayerProjection {
    pub fn get(&self, id: &PlayerRef) -> Option<Player> {
        self.0.lock().unwrap().players.get(id).cloned()
    }
}

impl Projection<PlayerRef, PlayerEvent> for InMemoryPlayerProjection {
    async fn checkpoint(&self) -> Result<u64, GameError> {
        Ok(self.0.lock().unwrap().checkpoint)
    }

    async fn apply(&self, events: &[Recorded<PlayerRef, PlayerEvent>]) -> Result<(), GameError> {
        let mut state = self.0.lock().unwrap();
        for recorded in events {
            if recorded.position <= state.checkpoint {
                continue;
            }
            let player = state.players.remove(&recorded.stream);
            if let Some(player) = Player::apply(player, &recorded.event) {
                state.players.insert(recorded.stream.clone(), player);
            }
            state.checkpoint = recorded.position;
        }
        Ok(())
    }
}

impl PlayerView for InMemoryPlayerProjection {
    async fn list(&self) -> Result<Vec<Player>, GameError> {
        Ok(self.0.lock().unwrap().players.values().cloned().collect())
    }
}

#[derive(Default)]
struct TreasuryTotal {
    checkpoint: u64,
    total: Money,
}

/// Money taken out of the game, summed from the treasury events of the ledger.
#[derive(Clone, Default)]
pub struct InMemoryTreasuryProjection(Arc<Mutex<TreasuryTotal>>);

impl Projection<TreasuryId, TreasuryEvent> for InMemoryTreasuryProjection {
    async fn checkpoint(&self) -> Result<u64, GameError> {
        Ok(self.0.lock().unwrap().checkpoint)
    }

    async fn apply(&self, events: &[Recorded<TreasuryId, TreasuryEvent>]) -> Result<(), GameError> {
        let mut state = self.0.lock().unwrap();
        for recorded in events {
            if recorded.position <= state.checkpoint {
                continue;
            }
            let TreasuryEvent::MoneyTaken { amount, .. } = recorded.event;
            state.total = Money(state.total.0.saturating_add(amount.0));
            state.checkpoint = recorded.position;
        }
        Ok(())
    }
}

impl TreasuryView for InMemoryTreasuryProjection {
    async fn total(&self) -> Result<Money, GameError> {
        Ok(self.0.lock().unwrap().total)
    }
}

#[derive(Clone)]
pub struct StaticCatalog(Arc<Vec<CatalogEntry>>);

impl StaticCatalog {
    pub fn new(entries: Vec<CatalogEntry>) -> Self {
        Self(Arc::new(entries))
    }
}

impl Catalog for StaticCatalog {
    async fn entry(&self, item_type: &ItemType) -> Result<Option<CatalogEntry>, GameError> {
        Ok(self.0.iter().find(|e| &e.item_type == item_type).cloned())
    }

    async fn entries(&self) -> Result<Vec<CatalogEntry>, GameError> {
        Ok(self.0.as_ref().clone())
    }
}

#[derive(Clone, Default)]
pub struct InMemoryHouseRegistry(Arc<Mutex<Vec<AuctionHouse>>>);

impl HouseRegistry for InMemoryHouseRegistry {
    async fn add(&self, house: AuctionHouse) -> Result<(), GameError> {
        self.0.lock().unwrap().push(house);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AuctionHouse>, GameError> {
        Ok(self.0.lock().unwrap().clone())
    }
}
