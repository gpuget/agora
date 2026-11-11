//! In-memory adapters of the game outbound ports. Cheap to clone: clones share state.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agora_domain::{AuctionHouse, Money, PlayerRef};
use event_sourcing::{Projection, Recorded, StoreError};
use ledger::{Account, AccountEvent, TreasuryEvent, TreasuryId};
use tcg_domain::{CardEntry, CardId, Collector, TcgError};
use tcg_spi::{Catalog, CollectorView, HouseRegistry, TreasuryView};

#[derive(Default)]
struct CollectorsState {
    checkpoint: u64,
    collectors: BTreeMap<PlayerRef, Collector>,
}

/// Collectors folded from the collector events of the ledger.
///
/// Reads return the state at the last applied event: wrap it in a
/// [`CaughtUpView`](crate::CaughtUpView) to read every event recorded before the call.
#[derive(Clone, Default)]
pub struct InMemoryCollectorProjection(Arc<Mutex<CollectorsState>>);

impl InMemoryCollectorProjection {
    /// The collector as last projected, `None` if its account is not projected yet.
    pub fn get(&self, id: &PlayerRef) -> Option<Collector> {
        self.0.lock().unwrap().collectors.get(id).cloned()
    }
}

impl Projection<PlayerRef, AccountEvent> for InMemoryCollectorProjection {
    async fn checkpoint(&self) -> Result<u64, StoreError> {
        Ok(self.0.lock().unwrap().checkpoint)
    }

    async fn apply(&self, events: &[Recorded<PlayerRef, AccountEvent>]) -> Result<(), StoreError> {
        let mut state = self.0.lock().unwrap();
        for recorded in events {
            if recorded.position <= state.checkpoint {
                continue;
            }
            let collector = state.collectors.remove(&recorded.stream);
            if let Some(collector) = Account::apply(collector, &recorded.event) {
                state.collectors.insert(recorded.stream.clone(), collector);
            }
            state.checkpoint = recorded.position;
        }
        Ok(())
    }
}

impl CollectorView for InMemoryCollectorProjection {
    async fn list(&self) -> Result<Vec<Collector>, TcgError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .collectors
            .values()
            .cloned()
            .collect())
    }
}

#[derive(Default)]
struct TreasuryTotal {
    checkpoint: u64,
    total: Money,
}

/// Dust taken out of the game, summed from the treasury events of the ledger.
#[derive(Clone, Default)]
pub struct InMemoryTreasuryProjection(Arc<Mutex<TreasuryTotal>>);

impl Projection<TreasuryId, TreasuryEvent> for InMemoryTreasuryProjection {
    async fn checkpoint(&self) -> Result<u64, StoreError> {
        Ok(self.0.lock().unwrap().checkpoint)
    }

    async fn apply(
        &self,
        events: &[Recorded<TreasuryId, TreasuryEvent>],
    ) -> Result<(), StoreError> {
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
    async fn total(&self) -> Result<Money, TcgError> {
        Ok(self.0.lock().unwrap().total)
    }
}

/// Catalog fixed at construction.
#[derive(Clone)]
pub struct StaticCatalog(Arc<Vec<CardEntry>>);

impl StaticCatalog {
    /// A catalog of `entries`. [`Catalog::entry`] returns the first entry of a card.
    pub fn new(entries: Vec<CardEntry>) -> Self {
        Self(Arc::new(entries))
    }
}

impl Catalog for StaticCatalog {
    async fn entry(&self, card: &CardId) -> Result<Option<CardEntry>, TcgError> {
        Ok(self.0.iter().find(|e| &e.card == card).cloned())
    }

    async fn entries(&self) -> Result<Vec<CardEntry>, TcgError> {
        Ok(self.0.as_ref().clone())
    }
}

/// House registry that lists the houses in the order they were added.
#[derive(Clone, Default)]
pub struct InMemoryHouseRegistry(Arc<Mutex<Vec<AuctionHouse>>>);

impl HouseRegistry for InMemoryHouseRegistry {
    async fn add(&self, house: AuctionHouse) -> Result<(), TcgError> {
        self.0.lock().unwrap().push(house);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AuctionHouse>, TcgError> {
        Ok(self.0.lock().unwrap().clone())
    }
}
