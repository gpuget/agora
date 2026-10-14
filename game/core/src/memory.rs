//! In-memory adapters of the game outbound ports. Cheap to clone: clones share state.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agora_domain::{AuctionHouse, ItemType, Money, PlayerRef};
use game_domain::{CatalogEntry, GameError, Player};
use game_spi::{Catalog, HouseRegistry, PlayerRepository, Treasury};

#[derive(Clone, Default)]
pub struct InMemoryPlayerRepository(Arc<Mutex<BTreeMap<PlayerRef, Player>>>);

impl InMemoryPlayerRepository {
    pub fn get(&self, id: &PlayerRef) -> Option<Player> {
        self.0.lock().unwrap().get(id).cloned()
    }
}

impl PlayerRepository for InMemoryPlayerRepository {
    async fn insert(&self, player: Player) -> Result<(), GameError> {
        self.0.lock().unwrap().insert(player.id.clone(), player);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Player>, GameError> {
        Ok(self.0.lock().unwrap().values().cloned().collect())
    }

    async fn update<T, F>(&self, id: &PlayerRef, change: F) -> Result<T, GameError>
    where
        T: Send,
        F: FnOnce(&mut Player) -> Result<T, GameError> + Send,
    {
        let mut players = self.0.lock().unwrap();
        let stored = players
            .get_mut(id)
            .ok_or_else(|| GameError::UnknownPlayer(id.clone()))?;
        // Work on a copy so that a failed change leaves the player untouched.
        let mut player = stored.clone();
        let value = change(&mut player)?;
        *stored = player;
        Ok(value)
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

#[derive(Clone, Default)]
pub struct InMemoryTreasury(Arc<Mutex<Money>>);

impl Treasury for InMemoryTreasury {
    async fn add(&self, amount: Money) -> Result<(), GameError> {
        let mut total = self.0.lock().unwrap();
        *total = Money(total.0.saturating_add(amount.0));
        Ok(())
    }

    async fn total(&self) -> Result<Money, GameError> {
        Ok(*self.0.lock().unwrap())
    }
}
