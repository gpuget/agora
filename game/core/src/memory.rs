//! In-memory adapters of the game outbound ports. Cheap to clone: clones share state.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agora_domain::{AuctionHouse, ItemType, Money, PlayerRef};
use event_sourcing::{Projection, Recorded, StoreError};
use game_domain::{CatalogEntry, GameError, Player, PlayerEvent, TreasuryEvent, TreasuryId};
use game_spi::{Catalog, HouseRegistry, PlayerView, TreasuryView};

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
    async fn checkpoint(&self) -> Result<u64, StoreError> {
        Ok(self.0.lock().unwrap().checkpoint)
    }

    async fn apply(&self, events: &[Recorded<PlayerRef, PlayerEvent>]) -> Result<(), StoreError> {
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
